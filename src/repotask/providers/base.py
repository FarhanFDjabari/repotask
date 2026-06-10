"""Prompt-only task provider adapter."""

from __future__ import annotations

from dataclasses import dataclass

from repotask.config.models import TaskProviderConfig


@dataclass(frozen=True)
class ProviderAdapter:
    config: TaskProviderConfig

    def task_url(self, task_id: str) -> str:
        return self.config.task_url(task_id)

    def reference(self, task_id: str) -> str:
        url = self.task_url(task_id)
        return f"[{task_id}]({url})" if url else task_id

    def reconciliation_instruction(self, task_id: str) -> str:
        connector = (
            f"Use the `{self.config.connector_hint}` connector if it is available."
            if self.config.connector_hint
            else "Use an available read-only connector or ask the user for live status."
        )
        return f"""Perform a READ-ONLY reconciliation for {self.config.display_name} `{task_id}`.

{connector}

Report the live provider status, local progress summary, discrepancies, and the single most likely
next manual action. Do not update provider status or automate any approval gate."""


def get_provider_adapter(config: TaskProviderConfig) -> ProviderAdapter:
    return ProviderAdapter(config)

