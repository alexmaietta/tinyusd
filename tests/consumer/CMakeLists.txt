cmake_minimum_required(VERSION 3.24)
project(tinyusd_consumer LANGUAGES C CXX)

find_package(tinyusd 0.1 CONFIG REQUIRED)

add_executable(consumer ../cpp/test_query.cpp)
target_link_libraries(consumer PRIVATE tinyusd::tinyusd)

get_target_property(_tinyusd_type tinyusd::tinyusd TYPE)
if(WIN32 AND _tinyusd_type STREQUAL "SHARED_LIBRARY")
  add_custom_command(TARGET consumer POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_RUNTIME_DLLS:consumer> $<TARGET_FILE_DIR:consumer>
    COMMAND_EXPAND_LISTS)
endif()

enable_testing()
add_test(NAME consumer COMMAND consumer "${CMAKE_CURRENT_SOURCE_DIR}/../data")
