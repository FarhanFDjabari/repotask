from repotask.templates import GENERATED_END, GENERATED_START, render_template


def test_template_insertion_is_idempotent_and_preserves_checklist() -> None:
    template = "# PR\n\n- [ ] Keep me\n\nTitle: {title}\n"
    first = render_template(template, "## Summary\n\nOne", {"title": "Fix"})
    second = render_template(first, "## Summary\n\nTwo", {"title": "Fix"})
    assert "- [ ] Keep me" in second
    assert second.count(GENERATED_START) == 1
    assert second.count(GENERATED_END) == 1
    assert "Two" in second
    assert "One" not in second

