import http from 'k6/http';
import { check } from 'k6';

export const options = {
  stages: [
    { duration: '1m', target: 100 },
    { duration: '2m', target: 200 },
    { duration: '3m', target: 300 },
    { duration: '3m', target: 600 },
    { duration: '30s', target: 0 },
  ],
};

const URL = 'http://localhost:8080/synapse/key_new_1';

export default function () {
  const res = http.get(URL);
  check(res, {
    'status is 200': (r) => r.status === 200,
  });
}
