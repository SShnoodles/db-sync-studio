import { useEffect, useMemo, useState } from "react";
import { Pagination } from "antd";
import type { ReactNode } from "react";

const sqlTokenPattern =
  /(--.*$)|('[^']*(?:''[^']*)*')|(`[^`]+`)|\b(SELECT|INSERT|INTO|VALUES|UPDATE|SET|DELETE|FROM|WHERE|AND|OR|NULL|IS|NOT|CREATE|ALTER|TABLE|ADD|DROP|MODIFY|CHANGE|PRIMARY|KEY|DEFAULT|CURRENT_TIMESTAMP|ON|DUPLICATE|ORDER|BY|GROUP|LIMIT|JOIN|LEFT|RIGHT|INNER|OUTER|AS|DISTINCT|INDEX|CONSTRAINT|UNIQUE|REFERENCES|AUTO_INCREMENT|COMMENT|COLUMN|RENAME|TO)\b|(\b\d+(?:\.\d+)?\b)/gi;

export function SqlCodePreview({ sql, pageSize = 100 }: { sql: string; pageSize?: number }) {
  const [page, setPage] = useState(1);
  const lines = useMemo(() => sql.split("\n"), [sql]);
  const total = lines.length;
  const start = (page - 1) * pageSize;
  const visibleLines = useMemo(
    () => lines.slice(start, start + pageSize),
    [lines, pageSize, start],
  );

  useEffect(() => {
    setPage(1);
  }, [sql]);

  return (
    <div className="sql-code-preview">
      <pre>
        {visibleLines.map((line, index) => {
          const lineNumber = start + index + 1;
          return (
            <span className="sql-line" key={`${lineNumber}-${line}`}>
              <span className="sql-line-number">{lineNumber}</span>
              <span className="sql-line-code">{line ? highlightSql(line) : " "}</span>
            </span>
          );
        })}
      </pre>
      {total > pageSize && (
        <div className="sql-code-pagination">
          <Pagination
            current={page}
            pageSize={pageSize}
            total={total}
            size="small"
            showSizeChanger={false}
            showTotal={(total) => `${total} lines`}
            onChange={setPage}
          />
        </div>
      )}
    </div>
  );
}

function highlightSql(line: string) {
  const nodes: ReactNode[] = [];
  let cursor = 0;
  line.replace(sqlTokenPattern, (match, comment, stringValue, identifier, keyword, numberValue, offset) => {
    if (offset > cursor) nodes.push(line.slice(cursor, offset));
    const className = comment
      ? "sql-token-comment"
      : stringValue
        ? "sql-token-string"
        : identifier
          ? "sql-token-identifier"
          : keyword
            ? "sql-token-keyword"
            : numberValue
              ? "sql-token-number"
              : undefined;
    nodes.push(
      <span className={className} key={`${offset}-${match}`}>
        {match}
      </span>,
    );
    cursor = offset + match.length;
    return match;
  });
  if (cursor < line.length) nodes.push(line.slice(cursor));
  return nodes;
}
