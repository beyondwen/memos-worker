interface MemoPageState {
  hasMore: boolean;
  loading: boolean;
  nextPageToken: string;
}

export function shouldAutoLoadNextMemoPage({
  hasMore,
  loading,
  nextPageToken,
}: MemoPageState): boolean {
  return hasMore && !loading && nextPageToken.trim().length > 0;
}
