"""Setup script for pymemchr_c - C extension build configuration."""

from setuptools import setup, Extension
import sys
import platform

# Determine compiler flags based on platform
extra_compile_args = []
extra_link_args = []

if sys.platform == 'darwin':
    # macOS - use clang
    extra_compile_args = ['-O3', '-Wall', '-Wextra']
    # Enable ARM NEON on Apple Silicon
    if platform.machine() == 'arm64':
        extra_compile_args.append('-march=armv8-a+simd')
    else:
        extra_compile_args.extend(['-msse2', '-msse4.1'])
elif sys.platform == 'linux':
    # Linux - gcc or clang
    extra_compile_args = ['-O3', '-Wall', '-Wextra', '-D_GNU_SOURCE']
    if platform.machine() == 'x86_64':
        extra_compile_args.extend(['-msse2', '-msse4.1'])
    elif platform.machine() == 'aarch64':
        extra_compile_args.append('-march=armv8-a+simd')
elif sys.platform == 'win32':
    # Windows - MSVC
    extra_compile_args = ['/O2', '/W3']

# Define the extension module
pymemchr_c_module = Extension(
    'pymemchr_c._pymemchr_c',
    sources=[
        'src/memchr.c',
        'src/pymemchr_c_module.c',
    ],
    include_dirs=['src'],
    extra_compile_args=extra_compile_args,
    extra_link_args=extra_link_args,
)

setup(
    ext_modules=[pymemchr_c_module],
)
