#[derive(Debug, thiserror::Error)]
pub enum LdtkError {
    #[error("An error occured while deserializing")]
    Json(#[from] serde_json::Error),
    #[error("One or more fields are missing in the LDTK file")]
    MissingFieldsForEntities,
    #[error("One or more fields are missing in the LDTK file")]
    MissingFieldsForLayers,
    #[error("One or more fields are missing in the LDTK file")]
    MissingFieldsForLevels,
    #[error("An unknown layer type was encountered")]
    UnknownLayerType(String),
    #[error("An unknown entity type was encountered")]
    UnknownEntityType(String),
}

pub type LdtkResult<T> = std::result::Result<T, LdtkError>;
