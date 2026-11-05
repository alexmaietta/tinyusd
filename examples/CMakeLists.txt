foreach(example mesh_import physics_import)
  add_executable(${example} cpp/${example}.cpp)
  target_link_libraries(${example} PRIVATE tinyusd::tinyusd)
  target_compile_features(${example} PRIVATE cxx_std_17)
  if(WIN32 AND NOT TINYUSD_STATIC AND TARGET tinyusd-shared)
    add_custom_command(TARGET ${example} POST_BUILD
      COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_FILE:tinyusd-shared> $<TARGET_FILE_DIR:${example}>)
  endif()
endforeach()

if(TINYUSD_BUILD_TESTS)
  set(_data "${PROJECT_SOURCE_DIR}/tests/data")
  add_test(NAME example_mesh_import COMMAND mesh_import "${_data}/material.usda")
  set_tests_properties(example_mesh_import PROPERTIES PASS_REGULAR_EXPRESSION
    "mesh /World/Quad: 4 corners, 2 triangles, normals yes, uvs yes.*first corner \\(world\\): -1.000 1.000 0.000.*base color texture .*tex.png \\(wrap repeat/repeat\\).*mesh /World/Floor: 6 corners, 2 triangles.*base color 0.80 0.10 0.10")
  add_test(NAME example_physics_import COMMAND physics_import "${_data}/physics.usda")
  set_tests_properties(example_physics_import PROPERTIES PASS_REGULAR_EXPRESSION
    "gravity 9.81 along 0 0 -1.*body /World/Base: mass 5.00 kg \\(kinematic\\) at 0.00 0.00 0.50.*body /World/Arm: mass 1.50 kg at 0.00 0.00 1.50.*collider /World/Ground: box \\[1.00 1.00 1.00\\] on <static>.*collider /World/Arm/Collider: capsule-Z \\[0.10 0.80 0.00\\] on /World/Arm.*collider /World/Arm/Visual: mesh-convexHull.*joint /World/Hinge: PhysicsRevoluteJoint /World/Base -> /World/Arm axis X limits \\[-90.0, 90.0\\]")
endif()
