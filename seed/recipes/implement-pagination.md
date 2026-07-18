---
id: implement-pagination
title: Implement Pagination
layer: recipe
stacks: [android, ios, flutter, web, typescript]
tags: [pagination, paging, list, infinite-scroll]
---
Replace these steps with the ones this project actually follows.

1. Confirm the API contract: cursor or offset, page size, and how the end is signalled.
2. Extend the repository, not the UI, with the paging call — see `repo-task fact repositories`.
3. Expose paging state (loading, appending, end-reached, error) from the state holder.
4. Keep the list stateless: it renders what it is given and asks for the next page.
5. Cover first page, next page, empty result, and error-then-retry in tests.
