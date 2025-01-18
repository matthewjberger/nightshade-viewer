import nightshade

conn = nightshade.Client()
conn.connect("ws://localhost:9124")

camera_id = nightshade.spawn_camera(conn, 0.0, 10.0, 15.0, "MainCamera")

LATTICE_SIZE = 4 
SPACING = 1.5
offset = (LATTICE_SIZE - 1) * SPACING / 2
for x in range(LATTICE_SIZE):
    for y in range(LATTICE_SIZE):
        for z in range(LATTICE_SIZE):
            # Calculate positions with offset
            pos_x = x * SPACING - offset
            pos_y = y * SPACING - offset
            pos_z = z * SPACING - offset
            
            # Spawn each cube
            cube_id = nightshade.spawn_cube(
                conn, 
                pos_x, pos_y, pos_z, 
                1.0,  # size
                f"Cube_{x}_{y}_{z}"  # unique name
            )

camera_ids = nightshade.request_cameras(conn)
print(f"Found cameras: {camera_ids}")

# Disconnect
conn.disconnect()