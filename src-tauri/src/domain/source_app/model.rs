use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDto {
    pub id: i64,
    pub name: String,
    pub icon: Option<String>,
    /// How many clips came from this app.
    pub count: i64,
}
