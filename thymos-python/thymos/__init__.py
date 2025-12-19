"""
Thymos Python bindings

IMPORTANT: Due to jemalloc TLS limitations in Python extensions, you MUST set
the MALLOC_CONF environment variable BEFORE importing this module:

    export MALLOC_CONF='background_thread:false'
    python -c "import thymos"

Or in your script, set it before any imports:
    import os
    os.environ['MALLOC_CONF'] = 'background_thread:false'
    import thymos  # Must be after setting the env var
"""

# Import the extension module
# Maturin creates thymos.abi3.so which Python loads as the 'thymos' module
# We need to import from the parent package to get the .so file
from .thymos import *

__all__ = ['Agent', 'Memory', 'AgentState', 'MemoryConfig', 'ThymosConfig']

__version__ = "0.1.0"

