use picoserve::{
    extract::State,
    response::{IntoResponse, Json},
};
use riot_rs::saga::{Reading, Sensor};

use crate::TempInput;

pub async fn temp(State(TempInput(temp)): State<TempInput>) -> impl IntoResponse {
    // FIXME: handle this unwrap
    let temp = temp.lock().await.read().await.unwrap().value().value;

    Json(JsonTemp { temp })
}

#[derive(serde::Serialize)]
struct JsonTemp {
    // Temperature in hundredths of degrees Celsius
    temp: i32,
}
