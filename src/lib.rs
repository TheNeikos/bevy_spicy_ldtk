use std::marker::PhantomData;

use bevy::{math::IVec2, utils::HashMap};
pub use bevy_spicy_ldtk_derive::ldtk;
use error::{LdtkError, LdtkResult};

pub mod error;

pub trait DeserializeLDtkLayers: Sized {
    type Entities: DeserializeLdtkEntities;

    fn deserialize_ldtk(instances: &[ldtk2::LayerInstance]) -> LdtkResult<Self>;
}

pub trait DeserializeLdtkEntities: Sized {
    fn deserialize_ldtk(instances: &[ldtk2::EntityInstance]) -> LdtkResult<Self>;
}

pub trait DeserializeLdtkFields: Sized {
    fn deserialize_ldtk(instances: &[ldtk2::FieldInstance]) -> LdtkResult<Self>;
}

pub trait DeserializeLdtk: Sized {
    fn deserialize_ldtk(ldtk: &ldtk2::Coordinate) -> LdtkResult<Self>;
}

#[derive(Debug)]
pub struct World<
    LevelFields: DeserializeLdtkFields,
    Entities: DeserializeLdtkEntities,
    Layers: DeserializeLDtkLayers<Entities = Entities>,
> {
    pub levels: Vec<Level<LevelFields, Entities, Layers>>,
    pub tilesets: HashMap<i64, Tileset>,
    _entities: PhantomData<Entities>,
}

impl<
        LevelFields: DeserializeLdtkFields,
        Entities: DeserializeLdtkEntities,
        Layers: DeserializeLDtkLayers<Entities = Entities>,
    > DeserializeLdtk for World<LevelFields, Entities, Layers>
{
    fn deserialize_ldtk(ldtk: &ldtk2::Ldtk) -> LdtkResult<Self> {
        let levels = ldtk
            .levels
            .iter()
            .map(Level::load)
            .collect::<LdtkResult<_>>()?;

        let tilesets = ldtk
            .defs
            .tilesets
            .iter()
            .map(|def| Ok((def.uid, Tileset::load(def)?)))
            .collect::<LdtkResult<_>>()?;

        Ok(World {
            levels,
            tilesets,
            _entities: PhantomData,
        })
    }
}

#[derive(Debug)]
pub struct Tile {
    pub flip_x: bool,
    pub flip_y: bool,
    pub position: ::bevy::math::IVec2,
    pub src: ::bevy::math::IVec2,
    pub id: i64,
}

#[derive(Debug)]
pub struct Tileset {
    pub grid_size: i64,
    pub ident: String,
    pub padding: i64,
    pub dimensions: ::bevy::math::IVec2,
    pub rel_path: String,
    pub id: i64,
}

impl Tileset {
    fn load(tileset: &ldtk2::TilesetDefinition) -> LdtkResult<Self> {
        let grid_size = tileset.tile_grid_size;
        let ident = tileset.identifier.clone();
        let padding = tileset.padding;
        let dimensions = IVec2::new(tileset.c_wid as i32, tileset.c_hei as i32);
        let rel_path = tileset.rel_path.clone();
        let id = tileset.uid;

        Ok(Tileset {
            grid_size,
            ident,
            padding,
            dimensions,
            rel_path,
            id,
        })
    }
}

#[derive(Debug)]
pub struct Level<
    LevelFields: DeserializeLdtkFields,
    Entities: DeserializeLdtkEntities,
    Layers: DeserializeLDtkLayers<Entities = Entities>,
> {
    pub background_color: ::bevy::render::color::Color,
    pub background_position: Option<::bevy::math::IVec2>,
    pub background_image_path: Option<String>,
    pub identifier: String,
    pub height: i64,
    pub width: i64,
    pub id: i64,
    pub world_position: ::bevy::math::IVec2,

    pub fields: LevelFields,
    pub layers: Layers,

    _entities: PhantomData<Entities>,
}

impl<
        LevelFields: DeserializeLdtkFields,
        Entities: DeserializeLdtkEntities,
        Layers: DeserializeLDtkLayers<Entities = Entities>,
    > Level<LevelFields, Entities, Layers>
{
    pub fn load(ldtk_level: &ldtk2::Level) -> LdtkResult<Self> {
        let fields = LevelFields::deserialize_ldtk(&ldtk_level.field_instances)?;
        // TODO: #1 Load from seperated ldtk files
        let layers = Layers::deserialize_ldtk(&ldtk_level.layer_instances.as_ref().unwrap())?;

        let background_color = bevy::prelude::Color::hex(&ldtk_level.bg_color[1..]).unwrap();
        let background_position = ldtk_level
            .bg_pos
            .as_ref()
            .map(|pos| IVec2::new(pos.top_left_px[0] as i32, pos.top_left_px[0] as i32));

        let background_image_path = ldtk_level.bg_rel_path.clone();
        let identifier = ldtk_level.identifier.clone();
        let height = ldtk_level.px_hei;
        let width = ldtk_level.px_wid;
        let id = ldtk_level.uid;
        let world_position = IVec2::new(ldtk_level.world_x as i32, ldtk_level.world_y as i32);

        Ok(Level {
            fields,
            layers,
            background_color,
            background_position,
            background_image_path,
            identifier,
            height,
            width,
            id,
            world_position,
            _entities: PhantomData,
        })
    }
}

#[derive(Debug)]
pub struct Layer<EntityFields> {
    pub height: i64,
    pub width: i64,
    pub grid_size: i64,
    pub opacity: f64,
    pub total_offset: ::bevy::math::IVec2,
    pub visible: bool,
    pub tileset_uid: Option<i64>,

    pub special: SpecialValues<EntityFields>,
}

impl<EntityFields: DeserializeLdtkEntities> Layer<EntityFields> {
    pub fn load(ldtk_layer: &ldtk2::LayerInstance) -> LdtkResult<Self> {
        let special = match ldtk_layer.layer_instance_type.as_str() {
            "IntGrid" => {
                let values = ldtk_layer.int_grid_csv.clone();
                SpecialValues::IntGrid { values }
            }
            "Entities" => {
                let entities = EntityFields::deserialize_ldtk(&ldtk_layer.entity_instances)?;

                SpecialValues::Entities(entities)
            }
            "Tiles" => {
                let tileset = ldtk_layer.tileset_def_uid;
                let tiles = vec![];

                SpecialValues::Tiles { tileset, tiles }
            }
            "AutoLayer" => SpecialValues::AutoLayer,
            unknown => return Err(LdtkError::UnknownLayerType(unknown.to_string())),
        };

        let height = ldtk_layer.c_hei;
        let width = ldtk_layer.c_wid;
        let grid_size = ldtk_layer.grid_size;
        let opacity = ldtk_layer.opacity;
        let total_offset = IVec2::new(
            ldtk_layer.px_total_offset_x as i32,
            ldtk_layer.px_total_offset_y as i32,
        );
        let visible = ldtk_layer.visible;
        let tileset_uid = ldtk_layer.tileset_def_uid;

        Ok(Layer {
            special,
            height,
            width,
            grid_size,
            opacity,
            total_offset,
            visible,
            tileset_uid,
        })
    }
}

#[derive(Debug)]
pub enum SpecialValues<Entities> {
    IntGrid {
        values: Vec<i64>,
    },
    Entities(Entities),
    Tiles {
        tileset: Option<i64>,
        tiles: Vec<Tile>,
    },
    AutoLayer,
}

#[doc(hidden)]
pub mod private {
    use crate::error::LdtkResult;
    use serde::de::DeserializeOwned;

    // Re-exports for the derive crate
    pub use bevy_spicy_aseprite::aseprite;
    pub use ldtk2;
    pub use serde::Deserialize;

    pub fn parse_field<T: DeserializeOwned>(field: &serde_json::Value) -> LdtkResult<T> {
        Ok(serde_json::from_value(field.clone())?)
    }
}
