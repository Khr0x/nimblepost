export function reorderRows<T>(rows: T[], from: number, to: number): T[] {
  if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to < 0 || from >= rows.length || to >= rows.length || from === to) return rows;
  const result = [...rows];
  const [row] = result.splice(from, 1);
  result.splice(to, 0, row);
  return result;
}
