use asteroids::{vector::Vector, world::WorldState};

#[test]
fn test_asteroid_collision_and_momentum() {
    let mut world = WorldState::new();

    let large_size = 1000.0;
    let small_size = 100.0;
    let distance_between = 50.0;

    world.spawn_asteroid(
        Vector { x: 0.0, y: 0.0 },
        Vector { x: 0.0, y: 0.0 },
        large_size,
    );
    world.spawn_asteroid(
        Vector {
            x: distance_between,
            y: 0.0,
        },
        Vector { x: 0.0, y: 0.0 },
        small_size,
    );

    let large_initial_pos = world.asteroids[0].pos();

    world.update(100.0);

    assert_eq!(
        world.asteroids.len(),
        1,
        "Asteroids should have merged into one"
    );

    let merged = &world.asteroids[0];

    let final_velocity = merged.vel();
    let velocity_magnitude = final_velocity.length();

    assert!(
        velocity_magnitude < 1e-5,
        "Final velocity magnitude should be essentially zero, got {}",
        velocity_magnitude
    );

    let final_pos = merged.pos();
    let distance_from_large = (final_pos - large_initial_pos).length();

    assert!(
        distance_from_large < distance_between / 2.0,
        "Final position should be close to larger asteroid, distance: {}",
        distance_from_large
    );

    let expected_size = large_size + small_size;
    let size_diff = (merged.size() - expected_size).abs();
    assert!(
        size_diff < 1e-6,
        "Merged asteroid should have combined size within floating point precision, expected {}, got {}, diff: {}",
        expected_size,
        merged.size(),
        size_diff
    );
}
