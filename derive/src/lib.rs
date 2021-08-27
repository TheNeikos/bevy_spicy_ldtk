use std::{path::PathBuf, str::FromStr};

use heck::{CamelCase, SnakeCase};
use ldtk2::{
    EntityDefinition, EnumDefinition, FieldDefinition, LayerDefinition, Ldtk, TilesetDefinition,
};
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

    let aseprite_tilesets = define_aseprite_tilesets(&path.value(), &ldtk.defs.tilesets);

    let expanded = quote! {
        #vis mod #name {

            pub mod enums {
                #custom_enums
            }

            #entities

            #levels

            pub const FILEPATH: &'static str = #path;

            pub mod aseprite_tilesets {
                #aseprite_tilesets
            }

            pub type Project = ::bevy_spicy_ldtk::World<
                LevelFields,
                ProjectEntities,
                Layers
            >;
        }
    };

    expanded.into()
}

fn define_aseprite_tilesets(path: &str, tilesets: &[TilesetDefinition]) -> TokenStream {
    let tilesets = tilesets.iter().map(|def| {
        if def.rel_path.ends_with(".aseprite") || def.rel_path.ends_with(".ase") {
            let mut path = PathBuf::from_str(path).unwrap();
            path.pop();
            path.push(&def.rel_path);

            let path = path.to_str().unwrap();
            println!("{}", path);

            let ident = format_ident!("{}", def.identifier.to_camel_case());

            quote! {
                ::bevy_spicy_ldtk::private::aseprite!(#ident, #path);
            }
        } else {
            quote! {}
        }
    });

    quote! {
        #(#tilesets)*
    }
}

fn define_levels(
    level_fields: &[FieldDefinition],
    level_layers: &[LayerDefinition],
) -> TokenStream {
    let ref custom_idents = level_fields
        .iter()
        .map(|def| &def.identifier)
        .collect::<Vec<_>>();
    let (ref custom_names, ref custom_types): (Vec<Ident>, Vec<TokenStream>) =
        define_fields(level_fields).into_iter().unzip();

    let layers = level_layers.iter().map(|def| {
        let ident = format_ident!("{}", def.identifier.to_snake_case());

        quote! {
            pub #ident: ::bevy_spicy_ldtk::Layer<ProjectEntities>
        }
    });

    let ref layer_names = level_layers
        .iter()
        .map(|def| format_ident!("{}", def.identifier.to_snake_case()))
        .collect::<Vec<_>>();
    let ref layer_idents = level_layers
        .iter()
        .map(|def| &def.identifier)
        .collect::<Vec<_>>();

    quote! {
        #[derive(Debug)]
        pub struct LevelFields {
            #(pub #custom_names: #custom_types),*
        }

        impl ::bevy_spicy_ldtk::DeserializeLdtkFields for LevelFields {
            fn deserialize_ldtk(instances: &[::bevy_spicy_ldtk::private::ldtk2::FieldInstance]) -> ::bevy_spicy_ldtk::error::LdtkResult<Self> {
                #(let mut #custom_names: Option<#custom_types> = None;)*

                #(
                    #custom_names = instances.iter().find(|field| field.identifier == #custom_idents)
                        .and_then(|field| field.value.as_ref())
                        .map(|value| ::bevy_spicy_ldtk::private::parse_field(value))
                        .transpose()?;
                )*


                match (#(#custom_names),*) {
                    (#(Some(#custom_names)),*) => {
                        Ok(LevelFields {
                            #(#custom_names),*
                        })
                    }
                    _ => Err(::bevy_spicy_ldtk::error::LdtkError::MissingFieldsForLevels)
                }
            }
        }

        #[derive(Debug)]
        pub struct Layers {
            #(#layers),*
        }


        impl ::bevy_spicy_ldtk::DeserializeLDtkLayers for Layers {
            type Entities = ProjectEntities;

            fn deserialize_ldtk(instances: &[::bevy_spicy_ldtk::private::ldtk2::LayerInstance]) -> ::bevy_spicy_ldtk::error::LdtkResult<Self> {
                #(let mut #layer_names: Option<_> = None;)*

                #(
                    #layer_names = instances.iter().find(|layer| layer.identifier == #layer_idents)
                        .map(|layer| ::bevy_spicy_ldtk::Layer::load(layer))
                        .transpose()?;
                )*


                match (#(#layer_names),*) {
                    (#(Some(#layer_names)),*) => {
                        Ok(Layers {
                            #(#layer_names),*
                        })
                    }
                    _ => Err(::bevy_spicy_ldtk::error::LdtkError::MissingFieldsForLayers)
                }
            }
        }
    }
}

fn define_enums(enums: &[EnumDefinition]) -> TokenStream {
    let enums = enums.iter().map(|def| {
        let ident = format_ident!("{}", def.identifier.to_camel_case());

        let fields = def.values.iter().map(|val| {
            let field_ident = format_ident!("{}", val.id.to_camel_case());

            quote! {#field_ident}
        });

        quote! {

            #[derive(Debug, ::bevy_spicy_ldtk::private::Deserialize)]
            pub enum #ident {
                #(#fields),*
            }
        }
    });

    quote! {
        #(#enums)*
    }
}

fn define_entities(ldtk_entities: &[EntityDefinition]) -> TokenStream {
    let entities = ldtk_entities.iter().map(|def| {
        let ident = format_ident!("{}", def.identifier.to_camel_case());

        let custom_ident = format_ident!("{}Fields", def.identifier.to_camel_case());

        let can_be_null = def.field_defs.iter().map(|def| def.can_be_null.clone());
        let custom_default = def.field_defs.iter().map(|def| if def.can_be_null { quote! { None } } else { quote!{ unreachable!() }});
        let custom_idents = def.field_defs.iter().map(|def| def.identifier.clone());
        let (custom_names, custom_types): (Vec<Ident>, Vec<TokenStream>) =
            define_fields(&def.field_defs).into_iter().unzip();

        quote! {
            #[derive(Debug)]
            pub struct #custom_ident {
                #(pub #custom_names: #custom_types),*
            }

            impl ::bevy_spicy_ldtk::DeserializeLdtkFields for #custom_ident {
                fn deserialize_ldtk(instances: &[::bevy_spicy_ldtk::private::ldtk2::FieldInstance]) -> ::bevy_spicy_ldtk::error::LdtkResult<Self> {
                    #(let #custom_names: #custom_types;)*

                    #(
                        let tmp: Option<Result<Option<#custom_types>,_>> = instances.iter().find(|field| field.identifier == #custom_idents)
                            .map(|field| field.value.as_ref())
                            .map(|value| value.map(|value| ::bevy_spicy_ldtk::private::parse_field(value)).transpose());

                        #custom_names = match tmp {
                            None => return Err(::bevy_spicy_ldtk::error::LdtkError::MissingFieldsForEntities),
                            Some(Err(e)) => return Err(e),
                            Some(Ok(val)) => if let Some(val) = val {
                                val
                            } else {
                                if !#can_be_null {
                                    return Err(::bevy_spicy_ldtk::error::LdtkError::MissingFieldsForEntities)
                                } else {
                                    #custom_default
                                }
                            },
                        };
                    )*
                    Ok(#custom_ident {
                        #(#custom_names,)*
                    })
                }
            }

            #[derive(Debug)]
            pub struct #ident {
                pub dimensions_px: ::bevy::math::IVec2,
                pub position_cell: ::bevy::math::IVec2,
                pub position_px: ::bevy::math::IVec2,
                pub pivot: ::bevy::math::Vec2,
                pub fields: #custom_ident,
            }

            impl #ident {
                fn load(entity: &::bevy_spicy_ldtk::private::ldtk2::EntityInstance, parent_size_grid: ::bevy::math::IVec2, parent_size_px: ::bevy::math::IVec2) -> ::bevy_spicy_ldtk::error::LdtkResult<Self> {
                    let dimensions_px = ::bevy::math::IVec2::new(entity.width as i32, entity.height as i32);
                    let position_cell = ::bevy::math::IVec2::new(entity.grid[0] as i32, parent_size_grid.y - entity.grid[1] as i32 - 1);
                    let pivot = ::bevy::math::Vec2::new(entity.pivot[0] as f32, 1.0 - entity.pivot[1] as f32);
                    let position_px = ::bevy::math::IVec2::new(entity.px[0] as i32, parent_size_px.y - entity.px[1] as i32 - 1);
                    let fields = <#custom_ident as ::bevy_spicy_ldtk::DeserializeLdtkFields>::deserialize_ldtk(&entity.field_instances)?;

                    Ok(#ident {
                        dimensions_px, position_cell, position_px, pivot, fields
                    })
                }
            }
        }
    });

    let entity_identifiers = ldtk_entities.iter().map(|def| &def.identifier);
    let (ref entity_group_names, ref entity_group_types): (Vec<Ident>, Vec<Ident>) = ldtk_entities
        .iter()
        .map(|def| {
            let ident = format_ident!("all_{}", def.identifier.to_snake_case());

            let custom_ident = format_ident!("{}", def.identifier.to_camel_case());

            (ident, custom_ident)
        })
        .unzip();

    quote! {
        #[derive(Debug)]
        pub struct ProjectEntities {
            #(pub #entity_group_names: Vec<#entity_group_types>),*
        }


        impl ::bevy_spicy_ldtk::DeserializeLdtkEntities for ProjectEntities {
            fn deserialize_ldtk(instances: &[::bevy_spicy_ldtk::private::ldtk2::EntityInstance], parent_size_grid: ::bevy::math::IVec2,  parent_size_px: ::bevy::math::IVec2) -> ::bevy_spicy_ldtk::error::LdtkResult<Self> {

                #(let mut #entity_group_names = vec![];)*

                for entity in instances {
                    match entity.identifier.as_str() {
                        #(#entity_identifiers => #entity_group_names .push(<#entity_group_types>::load(&entity, parent_size_grid, parent_size_px)?),)*
                        unknown => return Err(::bevy_spicy_ldtk::error::LdtkError::UnknownEntityType(unknown.to_string())),
                    }
                }

                Ok(
                    ProjectEntities {
                        #(#entity_group_names),*
                    }
                )
            }
        }

        #(#entities)*
    }
}

fn define_fields(field_defs: &[FieldDefinition]) -> Vec<(Ident, TokenStream)> {
    field_defs
        .iter()
        .map(|field| {
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
                "String" => quote! {String},
                "FilePath" => quote! {PathBuf},
                "Bool" => quote! {bool},
                "Color" => quote! {::bevy::render::color::Color},
                "Point" => quote! {::bevy::math::Vec2},
                name if name.starts_with("LocalEnum.") => {
                    let local_enum =
                        format_ident!("{}", name["LocalEnum.".len()..].to_camel_case());

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
                quote! {Vec< #kind >}
            } else {
                kind
            };

            let kind = if can_be_null {
                quote! {Option< #kind >}
            } else {
                kind
            };

            (name, kind)
        })
        .collect()
}
