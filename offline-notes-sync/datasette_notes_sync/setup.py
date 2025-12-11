from setuptools import setup

VERSION = "0.1.0"

setup(
    name="datasette-notes-sync",
    description="Datasette plugin for offline-first notes synchronization",
    author="Claude",
    url="https://github.com/simonw/research",
    license="Apache License, Version 2.0",
    version=VERSION,
    packages=["datasette_notes_sync"],
    entry_points={
        "datasette": [
            "notes_sync = datasette_notes_sync"
        ]
    },
    install_requires=["datasette"],
    python_requires=">=3.8",
)
