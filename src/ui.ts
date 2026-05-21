export function appHtml(): string {
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Memos Worker</title>
  <style>
    body {
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
      color: #242421;
      background: #f2f3ed;
    }
    main {
      width: min(420px, calc(100vw - 32px));
      padding: 24px;
      border: 1px solid #e5e2dc;
      border-radius: 8px;
      background: #fff;
      box-shadow: 0 12px 34px rgba(36,36,33,0.07);
    }
    h1 {
      margin: 0 0 8px;
      font-family: Georgia, "Times New Roman", serif;
      font-size: 24px;
    }
    p {
      margin: 0;
      color: #6f6a61;
      line-height: 1.7;
    }
  </style>
</head>
<body>
  <main>
    <h1>Memos Worker</h1>
    <p>前端静态资源未挂载。请运行 Web 构建并在 Worker 中启用 Assets 绑定。</p>
  </main>
</body>
</html>`;
}
