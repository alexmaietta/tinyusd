import os
import re
import shutil

from conan import ConanFile
from conan.errors import ConanInvalidConfiguration
from conan.tools.cmake import CMake, CMakeToolchain, cmake_layout
from conan.tools.files import copy, load, rmdir

required_conan_version = ">=2.0"

FAMILIES = ["shade", "lux", "skel", "physics", "vol", "media", "semantics"]


class TinyusdConan(ConanFile):
    """Builds tinyusd from this checkout with cargo, through the project's CMake build."""

    name = "tinyusd"
    description = "Small OpenUSD reader and writer with C, C++ and Python APIs"
    license = "BSD-3-Clause"
    url = "https://github.com/alexmaietta/tinyusd"
    homepage = "https://github.com/alexmaietta/tinyusd"
    topics = ("usd", "openusd", "3d", "scene-description", "importer")
    package_type = "library"
    settings = "os", "arch", "compiler", "build_type"
    options = {"shared": [True, False], "write": [True, False], **{f"with_{f}": [True, False] for f in FAMILIES}}
    default_options = {"shared": False, "write": True, **{f"with_{f}": True for f in FAMILIES}}

    @property
    def _root(self):
        return os.path.join(self.recipe_folder, "..", "..")

    def set_version(self):
        cmake = load(self, os.path.join(self._root, "CMakeLists.txt"))
        self.version = re.search(r"project\(tinyusd VERSION (\S+)", cmake).group(1)

    def export_sources(self):
        for pattern in ("CMakeLists.txt", "Cargo.toml", "Cargo.lock", "LICENSE", "cmake/*", "include/*", "licenses/*"):
            copy(self, pattern, self._root, self.export_sources_folder)
        copy(self, "crates/*", self._root, self.export_sources_folder, excludes=("*/tests/*", "*/__pycache__/*"))

    def layout(self):
        cmake_layout(self)

    def package_id(self):
        self.info.settings.rm_safe("compiler.cppstd")
        self.info.settings.rm_safe("compiler.libcxx")

    def validate_build(self):
        if shutil.which("cargo") is None:
            raise ConanInvalidConfiguration("tinyusd is written in Rust. Install a Rust toolchain (https://rustup.rs) so cargo is on PATH.")

    def generate(self):
        features = [f for f in FAMILIES if self.options.get_safe(f"with_{f}")]
        if self.options.write:
            features.append("write")
        tc = CMakeToolchain(self)
        tc.cache_variables["TINYUSD_STATIC"] = not self.options.shared
        tc.cache_variables["TINYUSD_FEATURES"] = ";".join(features)
        tc.cache_variables["TINYUSD_BUILD_TESTS"] = False
        tc.cache_variables["TINYUSD_BUILD_EXAMPLES"] = False
        tc.cache_variables["TINYUSD_INSTALL"] = True
        tc.generate()

    def build(self):
        cmake = CMake(self)
        cmake.configure()
        cmake.build()

    def package(self):
        CMake(self).install()
        rmdir(self, os.path.join(self.package_folder, "lib", "cmake"))

    def package_info(self):
        self.cpp_info.set_property("cmake_file_name", "tinyusd")
        self.cpp_info.set_property("cmake_target_name", "tinyusd::tinyusd")
        self.cpp_info.libs = ["tinyusd.dll" if self.settings.os == "Windows" and self.options.shared else "tinyusd"]
        if not self.options.shared:
            if self.settings.os == "Windows":
                self.cpp_info.system_libs = ["kernel32", "ntdll", "userenv", "ws2_32", "dbghelp"]
            elif self.settings.os == "Macos":
                self.cpp_info.system_libs = ["iconv", "m"]
            else:
                self.cpp_info.system_libs = ["gcc_s", "util", "rt", "pthread", "m", "dl"]
