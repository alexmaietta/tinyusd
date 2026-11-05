include(FetchContent)
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
include(TinyusdPlatform)

# A Corrosion provided by a package manager (vcpkg, Conan, conda) is used when one is found.
FetchContent_Declare(Corrosion GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git GIT_TAG v0.6.1 GIT_SHALLOW TRUE
  FIND_PACKAGE_ARGS 0.6 CONFIG)
FetchContent_MakeAvailable(Corrosion)

if(TINYUSD_STATIC)
  set(_tinyusd_crate_type staticlib)
  set(_tinyusd_impl tinyusd-static)
else()
  set(_tinyusd_crate_type cdylib)
  set(_tinyusd_impl tinyusd-shared)
endif()

corrosion_import_crate(
  MANIFEST_PATH "${PROJECT_SOURCE_DIR}/Cargo.toml"
  CRATES tinyusd-capi
  CRATE_TYPES ${_tinyusd_crate_type}
  PROFILE release
  NO_DEFAULT_FEATURES
  FEATURES ${TINYUSD_FEATURES}
)

# Rust links the dynamic CRT on MSVC by default. Match a static CRT when the build asks for one.
if(MSVC AND DEFINED CMAKE_MSVC_RUNTIME_LIBRARY AND NOT CMAKE_MSVC_RUNTIME_LIBRARY MATCHES "DLL")
  corrosion_add_target_rustflags(tinyusd -Ctarget-feature=+crt-static)
endif()

target_include_directories(tinyusd INTERFACE
  $<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/include>
  $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>)
target_compile_features(tinyusd INTERFACE cxx_std_17)
add_library(tinyusd::tinyusd ALIAS tinyusd)

if(TINYUSD_INSTALL)
  install(DIRECTORY "${PROJECT_SOURCE_DIR}/include/tinyusd" DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
  if(TINYUSD_STATIC)
    set(TINYUSD_LIB_KIND STATIC)
    install(FILES $<TARGET_FILE:${_tinyusd_impl}> DESTINATION "${CMAKE_INSTALL_LIBDIR}")
    set(TINYUSD_LIB_FILE "${CMAKE_INSTALL_LIBDIR}/${CMAKE_STATIC_LIBRARY_PREFIX}tinyusd${CMAKE_STATIC_LIBRARY_SUFFIX}")
    set(TINYUSD_IMPLIB_FILE "")
  else()
    set(TINYUSD_LIB_KIND SHARED)
    if(WIN32)
      install(FILES $<TARGET_FILE:${_tinyusd_impl}> DESTINATION "${CMAKE_INSTALL_BINDIR}")
      install(FILES $<TARGET_LINKER_FILE:${_tinyusd_impl}> DESTINATION "${CMAKE_INSTALL_LIBDIR}")
      set(TINYUSD_LIB_FILE "${CMAKE_INSTALL_BINDIR}/tinyusd.dll")
      if(MSVC)
        set(TINYUSD_IMPLIB_FILE "${CMAKE_INSTALL_LIBDIR}/tinyusd.dll.lib")
      else()
        set(TINYUSD_IMPLIB_FILE "${CMAKE_INSTALL_LIBDIR}/libtinyusd.dll.a")
      endif()
    else()
      install(FILES $<TARGET_FILE:${_tinyusd_impl}> DESTINATION "${CMAKE_INSTALL_LIBDIR}")
      set(TINYUSD_LIB_FILE "${CMAKE_INSTALL_LIBDIR}/${CMAKE_SHARED_LIBRARY_PREFIX}tinyusd${CMAKE_SHARED_LIBRARY_SUFFIX}")
      set(TINYUSD_IMPLIB_FILE "")
    endif()
  endif()
  tinyusd_static_system_libs(TINYUSD_SYSTEM_LIBS)
  set(TINYUSD_SONAME "")
  if(NOT TINYUSD_STATIC)
    if(APPLE)
      set(TINYUSD_SONAME "@rpath/libtinyusd.dylib")
    elseif(NOT WIN32)
      set(TINYUSD_SONAME "libtinyusd.so")
    endif()
  endif()

  set(_tinyusd_cmake_dir "${CMAKE_INSTALL_LIBDIR}/cmake/tinyusd")
  configure_package_config_file(
    "${PROJECT_SOURCE_DIR}/cmake/tinyusdConfig.cmake.in"
    "${PROJECT_BINARY_DIR}/tinyusdConfig.cmake"
    INSTALL_DESTINATION "${_tinyusd_cmake_dir}")
  write_basic_package_version_file(
    "${PROJECT_BINARY_DIR}/tinyusdConfigVersion.cmake"
    COMPATIBILITY SameMinorVersion)
  install(FILES
    "${PROJECT_BINARY_DIR}/tinyusdConfig.cmake"
    "${PROJECT_BINARY_DIR}/tinyusdConfigVersion.cmake"
    DESTINATION "${_tinyusd_cmake_dir}")
  install(FILES "${PROJECT_SOURCE_DIR}/LICENSE" DESTINATION "${CMAKE_INSTALL_DATADIR}/tinyusd")
  install(DIRECTORY "${PROJECT_SOURCE_DIR}/licenses" DESTINATION "${CMAKE_INSTALL_DATADIR}/tinyusd")
endif()
