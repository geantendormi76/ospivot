def test_core_package_importable() -> None:
    import ospivot

    assert ospivot.__name__ == "ospivot"
