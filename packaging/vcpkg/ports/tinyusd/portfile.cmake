set(VCPKG_POLICY_ONLY_RELEASE_CRT enabled)

if(VCPKG_TARGET_IS_WINDOWS)
    set(TINYUSD_OS windows)
elseif(VCPKG_TARGET_IS_OSX)
    set(TINYUSD_OS macos)
else()
    set(TINYUSD_OS linux)
endif()
if(VCPKG_TARGET_ARCHITECTURE STREQUAL "x64")
    set(TINYUSD_ARCH x86_64)
else()
    set(TINYUSD_ARCH aarch64)
endif()
if(VCPKG_LIBRARY_LINKAGE STREQUAL "dynamic")
    set(TINYUSD_KIND shared)
else()
    set(TINYUSD_KIND static)
endif()
set(TINYUSD_ARCHIVE "tinyusd-${VERSION}-${TINYUSD_OS}-${TINYUSD_ARCH}-${TINYUSD_KIND}")

# BEGIN HASHES
set(TINYUSD_SHA512_linux-x86_64-shared 0)
set(TINYUSD_SHA512_linux-x86_64-static 0)
set(TINYUSD_SHA512_linux-aarch64-shared 0)
set(TINYUSD_SHA512_linux-aarch64-static 0)
set(TINYUSD_SHA512_macos-x86_64-shared 0)
set(TINYUSD_SHA512_macos-x86_64-static 0)
set(TINYUSD_SHA512_macos-aarch64-shared 0)
set(TINYUSD_SHA512_macos-aarch64-static 0)
set(TINYUSD_SHA512_windows-x86_64-shared 0)
set(TINYUSD_SHA512_windows-x86_64-static 0)
set(TINYUSD_SHA512_windows-aarch64-shared 0)
set(TINYUSD_SHA512_windows-aarch64-static 0)
# END HASHES

vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/alexmaietta/tinyusd/releases/download/v${VERSION}/${TINYUSD_ARCHIVE}.tar.gz"
    FILENAME "${TINYUSD_ARCHIVE}.tar.gz"
    SHA512 "${TINYUSD_SHA512_${TINYUSD_OS}-${TINYUSD_ARCH}-${TINYUSD_KIND}}"
)
vcpkg_extract_source_archive(SOURCE_PATH ARCHIVE "${ARCHIVE}" NO_REMOVE_ONE_LEVEL)

file(INSTALL "${SOURCE_PATH}/include/" DESTINATION "${CURRENT_PACKAGES_DIR}/include")
file(INSTALL "${SOURCE_PATH}/lib/" DESTINATION "${CURRENT_PACKAGES_DIR}/lib")
if(EXISTS "${SOURCE_PATH}/bin")
    file(INSTALL "${SOURCE_PATH}/bin/" DESTINATION "${CURRENT_PACKAGES_DIR}/bin")
endif()
if(NOT VCPKG_BUILD_TYPE)
    file(INSTALL "${SOURCE_PATH}/lib/" DESTINATION "${CURRENT_PACKAGES_DIR}/debug/lib")
    if(EXISTS "${SOURCE_PATH}/bin")
        file(INSTALL "${SOURCE_PATH}/bin/" DESTINATION "${CURRENT_PACKAGES_DIR}/debug/bin")
    endif()
endif()
vcpkg_cmake_config_fixup(CONFIG_PATH lib/cmake/tinyusd)

file(INSTALL "${SOURCE_PATH}/share/tinyusd/licenses" DESTINATION "${CURRENT_PACKAGES_DIR}/share/${PORT}")
vcpkg_install_copyright(
    COMMENT "The library statically links third-party Rust crates. Their license files are in share/${PORT}/licenses, listed in share/${PORT}/licenses/INDEX.txt."
    FILE_LIST "${SOURCE_PATH}/share/tinyusd/LICENSE"
)
file(INSTALL "${CMAKE_CURRENT_LIST_DIR}/usage" DESTINATION "${CURRENT_PACKAGES_DIR}/share/${PORT}")
