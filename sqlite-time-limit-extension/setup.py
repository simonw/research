from setuptools import Extension, setup

module = Extension(
    "sqlite_time_limit._time_limit",
    sources=["src/sqlite_time_limit/time_limit.c"],
    libraries=["sqlite3"],
)

setup(
    ext_modules=[module],
)
