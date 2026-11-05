include(FetchContent)
include(TinyusdPlatform)

tinyusd_platform(_tinyusd_platform)
if(TINYUSD_STATIC)
  set(_tinyusd_kind static)
else()
  set(_tinyusd_kind shared)
endif()

set(_tinyusd_url "${TINYUSD_PREBUILT_URL}")
if(NOT _tinyusd_url)
  set(_tinyusd_url "${TINYUSD_RELEASE_URL}/v${PROJECT_VERSION}/tinyusd-${PROJECT_VERSION}-${_tinyusd_platform}-${_tinyusd_kind}.tar.gz")
endif()

set(_tinyusd_hash_arg)
if(TINYUSD_PREBUILT_SHA256)
  set(_tinyusd_hash_arg URL_HASH SHA256=${TINYUSD_PREBUILT_SHA256})
endif()

FetchContent_Declare(tinyusd_prebuilt URL "${_tinyusd_url}" ${_tinyusd_hash_arg} SOURCE_SUBDIR _none DOWNLOAD_EXTRACT_TIMESTAMP TRUE)
FetchContent_MakeAvailable(tinyusd_prebuilt)

set(CMAKE_FIND_PACKAGE_TARGETS_GLOBAL ON)
find_package(tinyusd CONFIG REQUIRED PATHS "${tinyusd_prebuilt_SOURCE_DIR}" NO_DEFAULT_PATH)
