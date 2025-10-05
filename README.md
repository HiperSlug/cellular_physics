# Cellular Physics
Last week I made a super simply cellular sand simulation but I wanted something that would be able to simulate more realisitc water. This project assigns each cell a mass and velocity and runs a simple physics simulation.

My end goal was to find something fast enough I could try to move it to 3d. However in this case I sacrificed too much accuracy for performance leading to non-water like behavior. \
This is fun janky physics simulation based on a cellular grid.

# Some fun things I did
I messed around with trying to compact information in each cell using invalid bit patterns.

I used `atomic`s for cheaper concurrency.

I used `union`s for the first time to dynamically change between `atomic` and `plain` data when neccecary.

I also used some raw pointers.
