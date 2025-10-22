from setuptools import setup, Extension
import glob
import os

# Get all C source files from cmark/src (excluding main.c)
cmark_src_dir = os.path.join(os.path.dirname(__file__), 'cmark', 'src')
cmark_ext_dir = os.path.join(os.path.dirname(__file__), 'cmark', 'extensions')
generated_src_dir = os.path.join(os.path.dirname(__file__), 'cmark', 'generated', 'unix')

# Collect all source files
sources = [
    'cmarkgfm_module.c'  # Our Python extension module
]

# Add all C files from cmark/src except main.c
for c_file in glob.glob(os.path.join(cmark_src_dir, '*.c')):
    if not c_file.endswith('main.c'):
        sources.append(c_file)

# Add all C files from cmark/extensions
for c_file in glob.glob(os.path.join(cmark_ext_dir, '*.c')):
    sources.append(c_file)

# Include directories
include_dirs = [
    cmark_src_dir,
    cmark_ext_dir,
    generated_src_dir,  # For config.h and generated headers
]

# Define the extension module
cmarkgfm_ext = Extension(
    '_cmarkgfm',
    sources=sources,
    include_dirs=include_dirs,
    extra_compile_args=[
        '-std=c99',
        '-DCMARK_STATIC_DEFINE',
        '-DCMARK_GFM_STATIC_DEFINE'
    ],
    define_macros=[
        ('CMARK_STATIC_DEFINE', '1'),
        ('CMARK_GFM_STATIC_DEFINE', '1'),
    ],
)

setup(
    name='cmarkgfm-pyodide',
    version='2025.10.22',
    description='Python bindings for cmark-gfm (without CFFI) for Pyodide',
    long_description='Python extension module for cmark-gfm built for Pyodide using Python C API instead of CFFI',
    author='Claude (AI Assistant)',
    license='MIT',
    ext_modules=[cmarkgfm_ext],
    py_modules=['cmarkgfm'],
    python_requires='>=3.9',
    classifiers=[
        'Programming Language :: Python :: 3',
        'Programming Language :: C',
        'Topic :: Text Processing :: Markup',
    ],
)
