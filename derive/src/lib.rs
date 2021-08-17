use heck::{CamelCase, SnakeCase};
use ldtk2::{Ldtk, TileInstance};
use proc_macro::TokenStream as TStream;
use proc_macro2::TokenStream;
use proc_macro_error::{abort, emit_call_site_error, proc_macro_error};
use quote::{format_ident, quote};
use syn::{parse::Parse, parse_macro_input, Ident, LitStr, Token, Visibility};

struct LdtkDeclaration {
    vis: Visibility,
    name: Ident,
    path: LitStr,
}

impl Parse for LdtkDeclaration {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;
        input.parse::<Token!(,)>()?;
        let path: LitStr = input.parse()?;

        Ok(LdtkDeclaration { vis, name, path })
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn ldtk(input: TStream) -> TStream {
    let LdtkDeclaration { vis, name, path } = parse_macro_input!(input as LdtkDeclaration);

    let ldtk = match Ldtk::from_path(path.value()) {
        Ok(ldtk) => ldtk,
        Err(err) => abort!(path, err),
    };

    let custom_enums = define_enums(&ldtk.defs.enums);

    let entities = define_entities(&ldtk.defs.entities);

    let levels = define_levels(&ldtk.defs.level_fields, &ldtk.defs.layers);

    let tilesets = define_tilesets(&ldtk.defs.tilesets);

    let world_levels = build_levels(&ldtk.levels, &ldtk.defs.tilesets);

    let expanded = quote! {
        #vis mod #name {

            pub mod enums {
                #custom_enums
            }

            #entities

            #levels

            pub mod tilesets {
                #tilesets
            }

            pub struct World {
                pub levels: &'static [Level],
            }

            pub const Project: World = World {
                levels: &[#world_levels],
            };
        }
    };

    expanded.into()
}

fn define_tilesets(tilesets: &[ldtk2::TilesetDefinition]) -> TokenStream {
    let tilesets = tilesets.iter().map(|tileset| {
        let ident = format_ident!("{}", tileset.identifier.to_camel_case());

        let grid_size = tileset.tile_grid_size;
        let identifier = tileset.identifier.as_str();
        let padding = tileset.padding;
        let dimensions = {
            let x = tileset.c_wid as i32;
            let y = tileset.c_hei as i32;

            quote! {
                ::bevy::math::const_ivec2!([#x, #y])
            }
        };
        let rel_path = &tileset.rel_path;
        let id = tileset.uid;

        quote! {
            pub const #ident: super::Tileset = super::Tileset {
                grid_size: #grid_size,
                ident: #identifier,
                padding: #padding,
                dimensions: #dimensions,
                rel_path: #rel_path,
                id: #id,
            };
        }
    });

    quote! {
        #(#tilesets)*
    }
}

fn build_levels(levels: &[ldtk2::Level], tilesets: &[ldtk2::TilesetDefinition]) -> TokenStream {
    let levels = levels.iter().map(|level| {
        let bg_color = color_from_str(&level.bg_color);

        let bg_position = if let Some(_pos) = level.bg_pos.as_ref() {
            quote! { None }
        } else {
            quote! { None }
        };

        let bg_path = quote! { None };

        let identifier = &level.identifier;

        let height = level.px_hei;
        let width = level.px_wid;

        let id = level.uid;

        let world_position = {
            let x = level.world_x as i32;
            let y = level.world_y as i32;

            quote! { ::bevy::math::const_ivec2!([#x, #y]) }
        };

        let level_fields = build_fields(&level.field_instances);

        let layer_fields = build_layers(level.layer_instances.as_ref(), &tilesets);

        quote! {
            Level {
                background_color: #bg_color,
                background_position: #bg_position,
                background_image_path: #bg_path,
                identifier: #identifier,
                height: #height,
                width: #width,
                id: #id,
                world_position: #world_position,

                fields: LevelFields {
                    #(#level_fields),*
                },
                layers: LevelLayers {
                    #(#layer_fields),*
                },
            }
        }
    });

    quote! {
        #(#levels),*
    }
}

fn build_layers(
    layer_instances: Option<&Vec<ldtk2::LayerInstance>>,
    tilesets: &[ldtk2::TilesetDefinition],
) -> Vec<TokenStream> {
    if let Some(layer_instances) = layer_instances {
        layer_instances
            .iter()
            .map(|layer| {
                let layer_ident = format_ident!("{}", layer.identifier.to_camel_case());
                let layer_kind = format_ident!("{}Layer", layer.identifier.to_camel_case());

                let height = layer.c_hei;
                let width = layer.c_wid;
                let grid_size = layer.grid_size;
                let opacity = layer.opacity;
                let total_offset = {
                    let x = layer.px_total_offset_x as i32;
                    let y = layer.px_total_offset_y as i32;

                    quote! { ::bevy::math::const_ivec2!([#x, #y]) }
                };
                let visible = layer.visible;

                let layer_fields: TokenStream = match layer.layer_instance_type.as_str() {
                    "IntGrid" => {
                        let values = &layer.int_grid_csv;
                        quote! {
                            values: &[#(#values),*],
                        }
                    }
                    "Entities" => {
                        quote! {}
                    }
                    "Tiles" => {
                        let tileset = {
                            let tileset = tilesets
                                .iter()
                                .find(|set| Some(set.uid) == layer.tileset_def_uid);

                            match tileset {
                                Some(tileset) => {
                                    let ident =
                                        format_ident!("{}", tileset.identifier.to_camel_case());

                                    quote! {&tilesets::#ident}
                                }
                                None => {
                                    emit_call_site_error!(format!(
                                        "Could not find tileset for layer {}",
                                        layer.identifier
                                    ));

                                    quote! {}
                                }
                            }
                        };
                        let tiles =
                            layer
                                .grid_tiles
                                .iter()
                                .map(|TileInstance { d: _, f, px, src, t }| {
                                    let flip_x = f & 0x1 != 0;
                                    let flip_y = f & 0x2 != 0;
                                    let position = {
                                        let x = px[0] as i32;
                                        let y = px[1] as i32;

                                        quote! {
                                            ::bevy::math::const_ivec2!([#x, #y])
                                        }
                                    };

                                    let src = {
                                        let x = src[0] as i32;
                                        let y = src[1] as i32;

                                        quote! {
                                            ::bevy::math::const_ivec2!([#x, #y])
                                        }
                                    };

                                    let id = *t;

                                    quote! {
                                        Tile {
                                            flip_x: #flip_x,
                                            flip_y: #flip_y,
                                            position: #position,
                                            src: #src,
                                            id: #id
                                        }
                                    }
                                });

                        quote! {
                            tileset: #tileset,
                            tiles: &[#(#tiles),*],
                        }
                    }
                    "AutoLayer" => quote! {},
                    layer_kind => {
                        emit_call_site_error!(format!("Unknown layer kind: {}", layer_kind));
                        quote! {}
                    }
                };
                quote! {
                    #layer_ident: #layer_kind {
                        height: #height,
                        width: #width,
                        grid_size: #grid_size,
                        opacity: #opacity,
                        total_offset: #total_offset,
                        visible: #visible,
                        #layer_fields
                    }
                }
            })
            .collect()
    } else {
        emit_call_site_error!("Split level files are not yet supported");
        vec![quote! {}]
    }
}

fn color_from_str(color: &str) -> TokenStream {
    if let (Ok(r), Ok(g), Ok(b)) = (
        color[1..3].parse::<u8>(),
        color[3..5].parse::<u8>(),
        color[5..7].parse::<u8>(),
    ) {
        quote! { ::bevy::render::color::Color::rgb(#r as f32 / 255., #g as f32 / 255., #b as f32 / 255.) }
    } else {
        emit_call_site_error!(format!("Invalid color: {}", color));
        quote! { () }
    }
}

fn build_fields(field_instances: &[ldtk2::FieldInstance]) -> Vec<TokenStream> {
    field_instances
        .iter()
        .map(|field| {
            let field_ident = format_ident!("{}", field.identifier.to_snake_case());
            let value = if let Some(value) = field.value.as_ref() {
                match field.field_instance_type.as_str() {
                    "Int" => {
                        let val = serde_json::from_value::<i64>(value.clone()).unwrap();

                        quote! { #val }
                    }
                    "Float" => {
                        let val = serde_json::from_value::<f64>(value.clone()).unwrap();

                        quote! { #val }
                    }
                    "String" => {
                        let val = serde_json::from_value::<String>(value.clone()).unwrap();
                        let val = val.as_str();

                        quote! { #val }
                    }
                    "FilePath" => {
                        let val = serde_json::from_value::<String>(value.clone()).unwrap();
                        let val = val.as_str();

                        quote! { #val }
                    }
                    "Bool" => {
                        let val = serde_json::from_value::<bool>(value.clone()).unwrap();

                        quote! { #val }
                    }
                    "Color" => {
                        let val = serde_json::from_value::<String>(value.clone()).unwrap();
                        let val = color_from_str(&val);

                        quote! { #val }
                    }
                    "Point" => {
                        let (x, y) = serde_json::from_value::<(i32, i32)>(value.clone()).unwrap();

                        quote! { ::bevy::math::const_ivec2!([#x, #y]) }
                    }
                    name if name.starts_with("LocalEnum.") => {
                        let _local_enum =
                            format_ident!("{}", name["LocalEnum.".len()..].to_camel_case());

                        let _val = serde_json::from_value::<i64>(value.clone()).unwrap();

                        quote! {}
                    }
                    kind => {
                        emit_call_site_error!(format!(
                            "Could not parse kind: \"{}\". Is this library outdated?",
                            kind
                        ));
                        quote! {}
                    }
                }
            } else {
                quote! { None }
            };

            quote! {
                #field_ident: #value
            }
        })
        .collect()
}

fn define_levels(
    level_fields: &[ldtk2::FieldDefinition],
    level_layers: &[ldtk2::LayerDefinition],
) -> TokenStream {
    let fields = define_fields(level_fields);

    let layer_defs = define_layers(level_layers);
    let layers = level_layers.iter().map(|def| {
        let ident = format_ident!("{}", def.identifier.to_camel_case());
        let ident_kind = format_ident!("{}Layer", def.identifier.to_camel_case());

        quote! {
            pub #ident: #ident_kind
        }
    });

    quote! {
        pub struct LevelFields {
            #fields
        }

        #layer_defs

        pub struct LevelLayers {
            #(#layers),*
        }

        pub struct Tile {
            pub flip_x: bool,
            pub flip_y: bool,
            pub position: ::bevy::math::IVec2,
            pub src: ::bevy::math::IVec2,
            pub id: i64,
        }

        pub struct Tileset {
            pub grid_size: i64,
            pub ident: &'static str,
            pub padding: i64,
            pub dimensions: ::bevy::math::IVec2,
            pub rel_path: &'static str,
            pub id: i64,
        }

        pub struct Level {
            pub background_color: ::bevy::render::color::Color,
            pub background_position: Option<::bevy::math::IVec2>,
            pub background_image_path: Option<&'static str>,
            pub identifier: &'static str,
            pub height: i64,
            pub width: i64,
            pub id: i64,
            pub world_position: ::bevy::math::IVec2,

            pub fields: LevelFields,
            pub layers: LevelLayers,
        }
    }
}

fn define_layers(layers: &[ldtk2::LayerDefinition]) -> TokenStream {
    let layers = layers.iter().map(|def| {
        let layer_ident = format_ident!("{}Layer", def.identifier.to_camel_case());

        let extra_fields = match def.layer_definition_type.as_str() {
            "IntGrid" => quote! {
                values: &'static [i64],
            },
            "Entities" => {
                quote! {}
            }
            "Tiles" => quote! {
                tileset: &'static Tileset,
                tiles: &'static [Tile]
            },
            "AutoLayer" => quote! {},
            layer_kind => {
                emit_call_site_error!(format!("Unknown layer kind: {}", layer_kind));
                quote! {}
            }
        };

        quote! {
            pub struct #layer_ident {
                pub height: i64,
                pub width: i64,
                pub grid_size: i64,
                pub opacity: f64,
                pub total_offset: ::bevy::math::IVec2,
                pub visible: bool,
                #extra_fields
            }
        }
    });

    quote! {
        #(#layers)*
    }
}

fn define_enums(enums: &[ldtk2::EnumDefinition]) -> TokenStream {
    let enums = enums.iter().map(|def| {
        let ident = format_ident!("{}", def.identifier.to_camel_case());

        let fields = def.values.iter().map(|val| {
            let field_ident = format_ident!("{}", val.id.to_camel_case());

            quote! {#field_ident}
        });

        quote! {
            pub enum #ident {
                #(#fields),*
            }
        }
    });

    quote! {
        #(#enums)*
    }
}

fn define_entities(entities: &[ldtk2::EntityDefinition]) -> TokenStream {
    let entities = entities.iter().map(|def| {
        let ident = format_ident!("{}", def.identifier.to_camel_case());

        let custom_ident = format_ident!("{}Fields", def.identifier.to_camel_case());

        let custom_fields = define_fields(&def.field_defs);

        quote! {
            pub struct #custom_ident {
                #custom_fields
            }

            pub struct #ident {
                pub width: i64,
                pub height: i64,
                pub fields: #custom_ident,
            }
        }
    });

    quote! {
        #(#entities)*
    }
}

fn define_fields(field_defs: &[ldtk2::FieldDefinition]) -> TokenStream {
    let fields = field_defs.iter().map(|field| {
        let is_array = field.field_definition_type.starts_with("Array<");
        let field_kind = if is_array {
            &field.field_definition_type["Array<".len()..field.field_definition_type.len() - 1]
        } else {
            &field.field_definition_type
        };

        let can_be_null = field.can_be_null;

        let name = format_ident!("{}", field.identifier.to_snake_case());

        let kind = match field_kind {
            "Int" => quote! {i64},
            "Float" => quote! {f64},
            "String" => quote! {&'static str},
            "FilePath" => quote! {&'static str},
            "Bool" => quote! {bool},
            "Color" => quote! {::bevy::render::color::Color},
            "Point" => quote! {::bevy::math::Vec2},
            name if name.starts_with("LocalEnum.") => {
                let local_enum = format_ident!("{}", name["LocalEnum.".len()..].to_camel_case());

                quote! {enums::#local_enum}
            }
            kind => {
                emit_call_site_error!(format!(
                    "Could not parse kind: \"{}\". Is this library outdated?",
                    kind
                ));
                quote! {}
            }
        };

        let kind = if is_array {
            quote! {&'static [ #kind ]}
        } else {
            kind
        };

        let kind = if can_be_null {
            quote! {Option< #kind >}
        } else {
            kind
        };

        quote! {
            pub #name: #kind
        }
    });

    quote! {
        #(#fields),*
    }
}
