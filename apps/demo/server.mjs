import { createServer } from "node:http";

const server = createServer((request, response) => {
  if (request.method !== "GET" || request.url !== "/health") {
    response.writeHead(404, { "content-type": "application/json" });
    response.end(JSON.stringify({ status: "not_found" }));
    return;
  }
  response.writeHead(200, { "content-type": "application/json" });
  response.end(JSON.stringify({ status: "ok" }));
});

server.listen(3000, "0.0.0.0", () => {
  process.stdout.write("ProjectBench demo endpoint listening on port 3000\n");
});
