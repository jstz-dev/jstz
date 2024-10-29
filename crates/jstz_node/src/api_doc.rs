use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(info(
    title = "Jstz Node",
    description = "JavaScript server runtime for Tezos Smart Rollups",
    license(
        name = "MIT",
        url = "https://github.com/jstz-dev/jstz/blob/main/LICENSE"
    ),
    contact(name = "Trilitech", email = "contact@trili.tech"),
))]
pub struct ApiDoc;
