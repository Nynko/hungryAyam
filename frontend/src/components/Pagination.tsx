import { createMemo, Show, For } from "solid-js";

interface PaginationProps {
  currentPage: number;
  totalPages: number;
  onPageChange: (page: number) => void;
}

export default function Pagination(props: PaginationProps) {
  const pageNumbers = createMemo(() => {
    const total = props.totalPages;
    const current = props.currentPage;
    const pages: (number | "ellipsis")[] = [];

    if (total <= 7) {
      for (let i = 1; i <= total; i++) pages.push(i);
      return pages;
    }

    pages.push(1);
    if (current > 3) pages.push("ellipsis");

    const start = Math.max(2, current - 1);
    const end = Math.min(total - 1, current + 1);
    for (let i = start; i <= end; i++) pages.push(i);

    if (current < total - 2) pages.push("ellipsis");
    pages.push(total);

    return pages;
  });

  return (
    <Show when={props.totalPages > 1}>
      <nav
        class="pagination is-centered mt-5"
        role="navigation"
        aria-label="pagination"
      >
        <button
          class="pagination-previous"
          disabled={props.currentPage <= 1}
          onClick={() => props.onPageChange(props.currentPage - 1)}
        >
          Previous
        </button>
        <button
          class="pagination-next"
          disabled={props.currentPage >= props.totalPages}
          onClick={() => props.onPageChange(props.currentPage + 1)}
        >
          Next
        </button>
        <ul class="pagination-list">
          <For each={pageNumbers()}>
            {(item) => (
              <Show
                when={item !== "ellipsis"}
                fallback={
                  <li>
                    <span class="pagination-ellipsis">&hellip;</span>
                  </li>
                }
              >
                <li>
                  <button
                    class="pagination-link"
                    classList={{
                      "is-current": item === props.currentPage,
                    }}
                    aria-label={`Go to page ${item}`}
                    aria-current={
                      item === props.currentPage ? "page" : undefined
                    }
                    onClick={() => props.onPageChange(item as number)}
                  >
                    {item}
                  </button>
                </li>
              </Show>
            )}
          </For>
        </ul>
      </nav>
    </Show>
  );
}