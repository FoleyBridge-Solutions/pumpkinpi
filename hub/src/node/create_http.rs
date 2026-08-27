use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct CreateNodeHttp {
    pub(crate) name: String,
}
