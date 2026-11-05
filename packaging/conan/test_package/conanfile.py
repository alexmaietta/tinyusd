import os

from conan import ConanFile
from conan.tools.build import can_run
from conan.tools.cmake import CMake, cmake_layout


class TinyusdTestConan(ConanFile):
    settings = "os", "arch", "compiler", "build_type"
    generators = "CMakeDeps", "CMakeToolchain", "VirtualRunEnv"

    def requirements(self):
        self.requires(self.tested_reference_str)

    def layout(self):
        cmake_layout(self)

    def build(self):
        variables = {"TINYUSD_TEST_WRITE": bool(self.dependencies["tinyusd"].options.write)}
        tests = os.path.normpath(os.path.join(self.source_folder, "..", "..", "..", "tests"))
        if os.path.isdir(os.path.join(tests, "cpp")):
            variables["TINYUSD_TESTS_DIR"] = tests.replace("\\", "/")
        cmake = CMake(self)
        cmake.configure(variables=variables, build_script_folder=os.path.join(self.source_folder, "..", "..", "test"))
        cmake.build()

    def test(self):
        if can_run(self):
            self.run(f"ctest --output-on-failure -C {self.settings.build_type}", cwd=self.build_folder, env="conanrun")
