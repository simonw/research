"""Basic tests for wazero package"""

import wazero


def test_import():
    """Test that the package can be imported"""
    assert wazero is not None


def test_exports():
    """Test that expected names are exported"""
    assert hasattr(wazero, "Runtime")
    assert hasattr(wazero, "Module")
    assert hasattr(wazero, "WazeroError")


def test_version():
    """Test version string"""
    assert hasattr(wazero, "__version__")
    assert isinstance(wazero.__version__, str)


def test_runtime_version():
    """Test that we can get the runtime version"""
    version = wazero.version()
    assert isinstance(version, str)
    assert len(version) > 0


def test_wazero_error():
    """Test WazeroError exception"""
    err = wazero.WazeroError("test error")
    assert str(err) == "test error"
    assert isinstance(err, Exception)


def test_runtime_is_class():
    """Test that Runtime is a class"""
    assert isinstance(wazero.Runtime, type)


def test_module_is_class():
    """Test that Module is a class"""
    assert isinstance(wazero.Module, type)
