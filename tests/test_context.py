from repotask.commands.context import _replace_description


def test_description_replacement_preserves_windows_paths() -> None:
    context = "# Task Context\n\n## Description\n\nTODO\n\n## Workflow References\n\n- None\n"
    description = "Source: C:\\Users\\developer\\requirement.md\n\nRequirement"

    updated = _replace_description(context, description)

    assert "Source: C:\\Users\\developer\\requirement.md" in updated
    assert "## Workflow References" in updated
