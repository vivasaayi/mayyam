const { createProxyMiddleware } = require('http-proxy-middleware');

module.exports = function(app) {
  // Determine the backend URL based on environment variables
  const proxyTarget = process.env.API_PROXY_TARGET || process.env.REACT_APP_API_URL || 'http://localhost:8080';
  const apiBaseUrl = process.env.REACT_APP_API_URL || '[same-origin]';

  console.log(`Frontend API base URL exposed to browser: ${apiBaseUrl}`);
  console.log(`Frontend proxy configured to connect to backend at: ${proxyTarget}`);

  app.use(
    '/api',
    createProxyMiddleware({
      target: proxyTarget,
      changeOrigin: true,
      secure: false,
      logLevel: 'debug',
      onProxyReq: (proxyReq, req, res) => {
        console.log(`Proxying ${req.method} ${req.url} to ${proxyTarget}`);
      },
      onError: (err, req, res) => {
        console.error('Proxy error:', err);
        res.status(500).send('Proxy error');
      }
    })
  );
};
