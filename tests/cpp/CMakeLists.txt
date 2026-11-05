add_executable(tinyusd_test_cpp test_query.cpp)
target_link_libraries(tinyusd_test_cpp PRIVATE tinyusd::tinyusd)
target_compile_features(tinyusd_test_cpp PRIVATE cxx_std_17)

add_executable(tinyusd_test_c test_query.c)
target_link_libraries(tinyusd_test_c PRIVATE tinyusd::tinyusd)
set_target_properties(tinyusd_test_c PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON)

if(NOT WIN32 AND TARGET tinyusd-shared)
  foreach(t tinyusd_test_cpp tinyusd_test_c)
    set_property(TARGET ${t} APPEND PROPERTY BUILD_RPATH "$<TARGET_FILE_DIR:tinyusd-shared>")
  endforeach()
endif()

if(WIN32 AND NOT TINYUSD_STATIC)
  foreach(t tinyusd_test_cpp tinyusd_test_c)
    add_custom_command(TARGET ${t} POST_BUILD
      COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_FILE:tinyusd-shared> $<TARGET_FILE_DIR:${t}>)
  endforeach()
endif()

set(_data "${PROJECT_SOURCE_DIR}/tests/data")
add_test(NAME cpp_query COMMAND tinyusd_test_cpp "${_data}")
add_test(NAME c_query COMMAND tinyusd_test_c "${_data}")
