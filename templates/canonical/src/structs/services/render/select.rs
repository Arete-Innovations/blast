pub struct SelectBuilder<T> {
    pub items: Vec<T>,
    pub label_field: Option<String>,
    pub value_field: Option<String>,
    pub name: Option<String>,
    pub placeholder: Option<String>,
    pub class_select: Option<String>,
    pub class_option: Option<String>,
    pub empty_text: String,
}
