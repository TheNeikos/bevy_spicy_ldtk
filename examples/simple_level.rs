use bevy_spicy_ldtk::ldtk;

ldtk! {pub levels, "assets/levels.ldtk"}

fn main() {
    let data = ldtk2::Ldtk::from_path(levels::FILEPATH).unwrap();

    let project: bevy_spicy_ldtk::World<_, _, _> = levels::Project::load(&data).unwrap();

    println!("ldtk file: {:?}", project);
}
