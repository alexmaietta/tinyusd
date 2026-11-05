cmake_minimum_required(VERSION 3.24)
project(tinyusd VERSION 0.2.0 LANGUAGES C CXX)

option(TINYUSD_USE_PREBUILT "Download a prebuilt tinyusd release instead of building with cargo" OFF)
option(TINYUSD_STATIC "Link tinyusd as a static library" OFF)
option(TINYUSD_BUILD_TESTS "Build tinyusd tests" ${PROJECT_IS_TOP_LEVEL})
option(TINYUSD_INSTALL "Generate tinyusd install rules" ${PROJECT_IS_TOP_LEVEL})
option(TINYUSD_BUILD_EXAMPLES "Build tinyusd examples" ${PROJECT_IS_TOP_LEVEL})
set(TINYUSD_FEATURES "shade;lux;skel;physics;vol;media;semantics;write" CACHE STRING "Schema families and the write API to compile in")
set(TINYUSD_RELEASE_URL "https://github.com/alexmaietta/tinyusd/releases/download" CACHE STRING "Base URL of prebuilt releases")
set(TINYUSD_PREBUILT_URL "" CACHE STRING "Explicit prebuilt archive URL, overrides TINYUSD_RELEASE_URL")
set(TINYUSD_PREBUILT_SHA256 "" CACHE STRING "Expected SHA256 of the prebuilt archive")

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

if(TINYUSD_USE_PREBUILT)
  include(TinyusdPrebuilt)
else()
  include(TinyusdSource)
endif()

if(TINYUSD_BUILD_TESTS)
  enable_testing()
  add_subdirectory(tests/cpp)
endif()

if(TINYUSD_BUILD_EXAMPLES)
  add_subdirectory(examples)
endif()
