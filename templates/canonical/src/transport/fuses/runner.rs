use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use dashmap::DashMap;
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection, RunQueryDsl};

use crate::{
    cata_log,
    ctx::Ctx,
    meltdown::{MeltDown, MeltType},
    structs::fuses::registry::{FuseFn, FuseRegistry},
    transport::fuses::schedule::schedule_from_row,
};

pub type Pool_ = Pool<AsyncPgConnection>;

use crate::structs::fuses::{table as fuses, FuseRow, NewFuseRow};

pub(crate) type FuseFnMap = Arc<HashMap<String, FuseFn>>;

async fn pool_conn(pool: &Pool_) -> Result<diesel_async::pooled_connection::deadpool::Object<AsyncPgConnection>, MeltDown> {
    pool.get().await.map_err(|e| MeltDown::db_connection(format!("fuses: failed to get pool conn: {}", e)))
}

pub async fn launch(pool: Pool_, registry: FuseRegistry) -> Result<(), MeltDown> {
    let mut conn = pool_conn(&pool).await?;
    let now = Utc::now();

    let existing: Vec<FuseRow> = fuses::table
        .select((
            fuses::id,
            fuses::name,
            fuses::flow_name,
            fuses::schedule_kind,
            fuses::schedule_spec,
            fuses::enabled,
            fuses::last_run_at,
            fuses::last_run_status,
            fuses::last_error,
            fuses::next_run_at,
            fuses::run_count,
            fuses::created_at,
            fuses::updated_at,
        ))
        .load::<FuseRow>(&mut conn)
        .await
        .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: load failed: {}", e)))?;

    let mut by_name: HashMap<String, FuseRow> = existing.into_iter().map(|r| (r.name.clone(), r)).collect();

    let mut fn_map: HashMap<String, FuseFn> = HashMap::new();

    for fuse in registry.iter() {
        let kind = fuse.schedule.kind_string();
        let spec = fuse.schedule.spec_string();
        let next = fuse.schedule.next_run_after(now);

        let Some(row) = by_name.remove(&fuse.name) else {
            let new_row = NewFuseRow {
                name: &fuse.name,
                flow_name: &fuse.flow_name,
                schedule_kind: kind,
                schedule_spec: &spec,
                next_run_at: next,
            };
            diesel::insert_into(fuses::table)
                .values(&new_row)
                .execute(&mut conn)
                .await
                .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: insert {} failed: {}", fuse.name, e)))?;
            cata_log!(Info, format!("fuse registered: {} ({})", fuse.name, spec));
            fn_map.insert(fuse.name.clone(), fuse.run_fn.clone());
            continue;
        };
        if row.schedule_kind != kind || row.schedule_spec != spec || row.flow_name != fuse.flow_name {
            diesel::update(fuses::table.filter(fuses::name.eq(&fuse.name)))
                .set((
                    fuses::flow_name.eq(&fuse.flow_name),
                    fuses::schedule_kind.eq(kind),
                    fuses::schedule_spec.eq(&spec),
                    fuses::next_run_at.eq(next),
                    fuses::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .await
                .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: update {} failed: {}", fuse.name, e)))?;
            cata_log!(Info, format!("fuse schedule updated: {} -> {}", fuse.name, spec));
        }

        fn_map.insert(fuse.name.clone(), fuse.run_fn.clone());
    }

    for (name, _) in by_name.iter() {
        cata_log!(Warning, format!("fuse '{}' present in DB but missing from code; leaving row in place", name));
    }

    drop(conn);

    let pool_for_loop = pool.clone();
    let fn_map_arc: FuseFnMap = Arc::new(fn_map);
    tokio::spawn(async move {
        run_loop(pool_for_loop, fn_map_arc).await;
    });

    Ok(())
}

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

pub(crate) async fn run_loop(pool: Pool_, fn_map: FuseFnMap) {
    let running: Arc<DashMap<String, ()>> = Arc::new(DashMap::new());

    loop {
        if let Err(e) = poll_once(&pool, &fn_map, &running).await {
            cata_log!(Error, format!("fuse runner poll failed: {}", e.details));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn poll_once(pool: &Pool_, fn_map: &FuseFnMap, running: &Arc<DashMap<String, ()>>) -> Result<(), MeltDown> {
    let mut conn = pool_conn(pool).await?;
    let now = Utc::now();

    let due: Vec<FuseRow> = fuses::table
        .filter(fuses::enabled.eq(true))
        .filter(fuses::next_run_at.le(now))
        .select((
            fuses::id,
            fuses::name,
            fuses::flow_name,
            fuses::schedule_kind,
            fuses::schedule_spec,
            fuses::enabled,
            fuses::last_run_at,
            fuses::last_run_status,
            fuses::last_error,
            fuses::next_run_at,
            fuses::run_count,
            fuses::created_at,
            fuses::updated_at,
        ))
        .load::<FuseRow>(&mut conn)
        .await
        .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: due query failed: {}", e)))?;

    drop(conn);

    for row in due {
        if running.contains_key(&row.name) {
            continue;
        }

        let Some(run_fn) = fn_map.get(&row.name).cloned() else {
            continue;
        };

        running.insert(row.name.clone(), ());
        let pool_for_task = pool.clone();
        let running_for_task = running.clone();
        let row_name = row.name.clone();

        tokio::spawn(async move {
            let res = run_fuse(pool_for_task, row, run_fn).await;
            running_for_task.remove(&row_name);
            if let Err(e) = res {
                cata_log!(Error, format!("fuse '{}' supervisor error: {}", row_name, e.details));
            }
        });
    }

    Ok(())
}

async fn run_fuse(pool: Pool_, row: FuseRow, run_fn: FuseFn) -> Result<(), MeltDown> {
    let started = Utc::now();
    cata_log!(Info, format!("fuse_run_started name={} attempt={}", row.name, row.run_count + 1));

    let next_at = match schedule_from_row(&row.schedule_kind, &row.schedule_spec) {
        Some(sched) => sched.next_run_after(Utc::now()),
        None => {
            let err_msg = format!("schedule_parse_error: kind='{}' spec='{}'", row.schedule_kind, row.schedule_spec);
            let mut conn = pool_conn(&pool).await?;
            diesel::update(fuses::table.filter(fuses::id.eq(row.id)))
                .set((
                    fuses::last_run_status.eq(Some("schedule_parse_error".to_string())),
                    fuses::last_error.eq(Some(err_msg.clone())),
                    fuses::enabled.eq(false),
                    fuses::updated_at.eq(Utc::now()),
                ))
                .execute(&mut conn)
                .await
                .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: mark-schedule-parse-error failed: {}", e)))?;
            cata_log!(Error, format!("fuse_disabled name={} reason='{}'", row.name, err_msg));
            return Ok(());
        }
    };

    {
        let mut conn = pool_conn(&pool).await?;
        diesel::update(fuses::table.filter(fuses::id.eq(row.id)))
            .set((fuses::last_run_status.eq(Some("running".to_string())), fuses::last_run_at.eq(started), fuses::updated_at.eq(started)))
            .execute(&mut conn)
            .await
            .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: mark-running failed: {}", e)))?;
    }

    let ctx = Ctx::system(pool.clone());
    let result = run_fn(&ctx).await;
    let finished = Utc::now();
    let duration_ms = (finished - started).num_milliseconds().max(0);

    let mut conn = pool_conn(&pool).await?;
    match result {
        Ok(()) => {
            diesel::update(fuses::table.filter(fuses::id.eq(row.id)))
                .set((
                    fuses::last_run_status.eq(Some("ok".to_string())),
                    fuses::last_error.eq::<Option<String>>(None),
                    fuses::run_count.eq(fuses::run_count + 1),
                    fuses::next_run_at.eq(next_at),
                    fuses::updated_at.eq(finished),
                ))
                .execute(&mut conn)
                .await
                .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: mark-ok failed: {}", e)))?;
            cata_log!(Info, format!("fuse_run_succeeded name={} duration_ms={}", row.name, duration_ms));
            Ok(())
        }
        Err(meltdown) => {
            let err_msg = meltdown.log_message();
            diesel::update(fuses::table.filter(fuses::id.eq(row.id)))
                .set((
                    fuses::last_run_status.eq(Some("error".to_string())),
                    fuses::last_error.eq(Some(err_msg.clone())),
                    fuses::next_run_at.eq(next_at),
                    fuses::updated_at.eq(finished),
                ))
                .execute(&mut conn)
                .await
                .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("fuses: mark-error failed: {}", e)))?;
            cata_log!(
                Error,
                format!("fuse_run_failed name={} duration_ms={} error_type={} error_message={}", row.name, duration_ms, meltdown.melt_type_str(), err_msg)
            );
            Ok(())
        }
    }
}
