
use rand::Rng;
use std::future::Future;
use std::time::{Duration, Instant};

use crate::{cata_log, meltdown::MeltDown};

pub trait RetryPolicy {
    fn max_attempts(&self) -> usize;

    fn delay(&self, attempt: usize) -> Duration;
}

#[derive(Debug, Clone)]
pub struct ExpBackoff {
    max_attempts: usize,
    base: Duration,
    cap: Option<Duration>,
    jitter: bool,
}

impl ExpBackoff {
    pub fn new(max_attempts: usize, base: Duration) -> Self {
        Self {
            max_attempts,
            base,
            cap: None,
            jitter: false,
        }
    }

    pub fn with_jitter(mut self) -> Self {
        self.jitter = true;
        self
    }

    pub fn with_cap(mut self, cap: Duration) -> Self {
        self.cap = Some(cap);
        self
    }
}

impl RetryPolicy for ExpBackoff {
    fn max_attempts(&self) -> usize {
        self.max_attempts
    }

    fn delay(&self, attempt: usize) -> Duration {
        let shift = attempt.saturating_sub(1).min(32) as u32;
        let factor: u64 = 1u64 << shift;
        let mut delay = self.base.saturating_mul(factor.min(u32::MAX as u64) as u32);

        self.cap.map(|cap| {
            if delay > cap {
                delay = cap;
            }
        });

        if self.jitter {
            let nanos = delay.as_nanos() as i128;
            let jitter_range = nanos / 4;
            if jitter_range > 0 {
                let mut rng = rand::thread_rng();
                let offset: i128 = rng.gen_range(-jitter_range..=jitter_range);
                let adjusted = (nanos + offset).max(0) as u128;
                delay = Duration::from_nanos(adjusted.min(u64::MAX as u128) as u64);
            }
        }

        delay
    }
}

#[derive(Debug, Clone)]
pub struct FixedDelay {
    max_attempts: usize,
    delay: Duration,
}

impl FixedDelay {
    pub fn new(max_attempts: usize, delay: Duration) -> Self {
        Self { max_attempts, delay }
    }
}

impl RetryPolicy for FixedDelay {
    fn max_attempts(&self) -> usize {
        self.max_attempts
    }
    fn delay(&self, _attempt: usize) -> Duration {
        self.delay
    }
}

#[derive(Debug, Clone)]
pub struct Immediate {
    max_attempts: usize,
}

impl Immediate {
    pub fn new(max_attempts: usize) -> Self {
        Self { max_attempts }
    }
}

impl RetryPolicy for Immediate {
    fn max_attempts(&self) -> usize {
        self.max_attempts
    }
    fn delay(&self, _attempt: usize) -> Duration {
        Duration::ZERO
    }
}

pub struct Crank<P>
where
    P: RetryPolicy,
{
    policy: P,
    classifier: Option<Box<dyn FnMut(&MeltDown) -> bool + Send>>,
    deadline: Option<Duration>,
    on_attempt: Option<Box<dyn FnMut(usize, &MeltDown) + Send>>,
    on_giveup: Option<Box<dyn FnMut(usize, &MeltDown) + Send>>,
}

impl<P> Crank<P>
where
    P: RetryPolicy,
{
    pub fn new(policy: P) -> Self {
        Self {
            policy,
            classifier: None,
            deadline: None,
            on_attempt: None,
            on_giveup: None,
        }
    }

    pub fn classify<C>(mut self, classifier: C) -> Self
    where
        C: FnMut(&MeltDown) -> bool + Send + 'static,
    {
        self.classifier = Some(Box::new(classifier));
        self
    }

    pub fn retry_only_transient(self) -> Self {
        self.classify(|e: &MeltDown| e.is_transient())
    }

    pub fn retry_all(self) -> Self {
        self.classify(|_: &MeltDown| true)
    }

    pub fn deadline(mut self, deadline: Duration) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn on_attempt<F>(mut self, hook: F) -> Self
    where
        F: FnMut(usize, &MeltDown) + Send + 'static,
    {
        self.on_attempt = Some(Box::new(hook));
        self
    }

    pub fn on_giveup<F>(mut self, hook: F) -> Self
    where
        F: FnMut(usize, &MeltDown) + Send + 'static,
    {
        self.on_giveup = Some(Box::new(hook));
        self
    }

    pub async fn run<T, F, Fut>(mut self, mut f: F) -> Result<T, MeltDown>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, MeltDown>>,
    {
        let Some(mut classifier) = self.classifier.take() else {
            panic!("Crank::run called without a classifier; call .classify(...) first");
        };

        let max_attempts = self.policy.max_attempts().max(1);
        let start = Instant::now();

        let mut attempt_no: usize = 1;
        let mut last_err: MeltDown;

        loop {
            match f().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    cata_log!(Debug, format!("Crank attempt {} failed: {}", attempt_no, err));
                    last_err = err;
                }
            }

            let retryable = classifier(&last_err);
            if !retryable {
                return Err(last_err);
            }

            if attempt_no >= max_attempts {
                self.on_giveup.as_mut().map(|hook| hook(attempt_no, &last_err));
                return Err(last_err);
            }

            let delay = match last_err.retry_after {
                Some(d) => d,
                None => self.policy.delay(attempt_no),
            };

            let timed_out = self
                .deadline
                .is_some_and(|budget| start.elapsed() + delay >= budget);
            if timed_out {
                self.on_giveup.as_mut().map(|hook| hook(attempt_no, &last_err));
                return Err(last_err);
            }

            attempt_no += 1;
            self.on_attempt.as_mut().map(|hook| hook(attempt_no, &last_err));

            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
        }
    }
}

impl Crank<Immediate> {
    pub fn none() -> Self {
        Self {
            policy: Immediate::new(1),
            classifier: Some(Box::new(|_| false)),
            deadline: None,
            on_attempt: None,
            on_giveup: None,
        }
    }
}

impl Crank<ExpBackoff> {
    pub fn backoff(max_attempts: usize, base: Duration) -> Self {
        Self {
            policy: ExpBackoff::new(max_attempts, base),
            classifier: Some(Box::new(|e: &MeltDown| !e.is_permanent())),
            deadline: None,
            on_attempt: None,
            on_giveup: None,
        }
    }
}

impl Crank<FixedDelay> {
    pub fn fixed(max_attempts: usize, delay: Duration) -> Self {
        Self {
            policy: FixedDelay::new(max_attempts, delay),
            classifier: Some(Box::new(|e: &MeltDown| !e.is_permanent())),
            deadline: None,
            on_attempt: None,
            on_giveup: None,
        }
    }
}
