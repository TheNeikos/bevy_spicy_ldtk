use bevy_spicy_ldtk::{ldtk, DeserializeLdtk};

ldtk! {pub levels, "assets/levels.ldtk"}

fn main() {
    let data = ldtk2::Ldtk::from_path(levels::FILEPATH).unwrap();

    let project: bevy_spicy_ldtk::World<_, _, _> = levels::Project::deserialize_ldtk(&data).unwrap();

    println!("ldtk file: {:?}", project);
}
