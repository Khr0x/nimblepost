import { createServer } from 'node:http';

const users = [{ id: 1, name: 'Ada' }, { id: 2, name: 'Linus' }];
createServer((request, response) => {
  const url = new URL(request.url, 'http://127.0.0.1:3000');
  const found = request.method === 'GET' && url.pathname === '/users';
  response.writeHead(found ? 200 : 404, { 'Content-Type': 'application/json', 'X-Demo': 'nimblepost' });
  response.end(JSON.stringify(found ? { users: users.slice(0, Number(url.searchParams.get('limit') ?? 5)) } : { error: 'Not found' }, null, 2));
}).listen(3000, '127.0.0.1', () => console.log('Demo API: http://127.0.0.1:3000/users'));
