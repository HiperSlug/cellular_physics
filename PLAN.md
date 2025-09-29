# Phases
Phase 1: "Forces" mutate each cells velocity based on gravity and normal force etc.
Phase 2: Simulation. For every single cell compute the path to the target. Go as far as it can.

# Cell structure
```rust
struct Cell {
	is_some: bool,
	velocity: I8Vec2,
	mass: u8,
}
```

In reality though velocity will be a single byte. 
```rust
struct Cell {
	cell_type: 2 bits, 7..5
	vel_x: 3 bits, 5..3
	vel_y: 3 bits, 3..0
}
```