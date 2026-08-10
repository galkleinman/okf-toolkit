---
type: GitHub Action
title: okf-validate-action
description: A composite action that downloads the okf binary and validates a bundle in any repository's CI.
tags: [architecture, ci, distribution]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Responsibility

Runs `okf validate` against a path in a consuming repository, in a few lines of
workflow YAML.

# Why a composite action, not Docker

A Docker action pays an image build or pull on every run, in every consuming
repository. A composite action downloads a prebuilt static binary from the
release matching the requested version and caches it. The binary is the reason
this is fast, and a Docker wrapper would discard that advantage.

# Dogfooding

This repository runs the action against its own [knowledge bundle](../index.md)
on every pull request, so the action and the bundle are both exercised by the
project that publishes them. A broken bundle fails CI exactly as broken code
would.
