"""Shared HTTP plumbing for REST connectors."""

from __future__ import annotations

from typing import Any

import httpx

from repotask.errors import RepoTaskError

TIMEOUT = 20.0


def get_json(
    url: str,
    headers: dict[str, str],
    params: dict[str, Any] | None = None,
    auth: tuple[str, str] | None = None,
) -> dict[str, Any]:
    """GET and decode JSON, converting transport and status failures into user errors."""
    try:
        response = httpx.get(
            url, headers=headers, params=params, auth=auth, timeout=TIMEOUT, follow_redirects=True
        )
    except httpx.HTTPError as error:
        raise RepoTaskError(f"Request to {url} failed: {error}") from error
    if response.status_code == 401 or response.status_code == 403:
        raise RepoTaskError(
            f"{url} rejected the credentials ({response.status_code}). Check your token scope."
        )
    if response.status_code == 404:
        raise RepoTaskError(f"Not found: {url}")
    if response.status_code >= 400:
        raise RepoTaskError(f"{url} returned {response.status_code}: {response.text[:200]}")
    try:
        payload = response.json()
    except ValueError as error:
        raise RepoTaskError(f"{url} did not return JSON.") from error
    return payload if isinstance(payload, dict) else {"items": payload}


def base_url(configured: str, fallback: str, system: str) -> str:
    url = (configured or fallback).rstrip("/")
    if not url:
        raise RepoTaskError(
            f"Connector '{system}' needs a base_url in .repo-task/config.yaml."
        )
    return url
