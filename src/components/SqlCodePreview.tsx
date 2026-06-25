import type { ReactNode } from "react";

const sqlTokenPattern =
  /(--.*$)|('[^']*(?:''[^']*)*')|(`[^`]+`)|\b(SELECT|INSERT|INTO|VALUES|UPDATE|SET|DELETE|FROM|WHERE|AND|OR|NULL|IS|NOT|CREATE|ALTER|TABLE|ADD|DROP|MODIFY|CHANGE|PRIMARY|KEY|DEFAULT|CURRENT_TIMESTAMP|ON|DUPLICATE|ORDER|BY|GROUP|LIMIT|JOIN|LEFT|RIGHT|INNER|OUTER|AS|DISTINCT|INDEX|CONSTRAINT|UNIQUE|REFERENCES|AUTO_INCREMENT|COMMENT|COLUMN|RENAME|TO)\b|(\b\d+(?:\.\d+)?\b)/gi;

export function SqlCodePreview({ sql }: { sql: string }) {
  return (
    <pre>
      {sql.split("\n").map((line, index) => (
        <span className="sql-line" key={`${index}-${line}`}>
          <span className="sql-line-number">{index + 1}</span>
          <span className="sql-line-code">{line ? highlightSql(line) : " "}</span>
        </span>
      ))}
    </pre>
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
