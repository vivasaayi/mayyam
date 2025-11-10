// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


import React, { useState, useContext } from 'react';
import { AuthContext } from '../../context/AuthContext';
import { Card, CardHeader, CardBody, Button, Alert } from 'reactstrap';
import { jwtDecode } from 'jwt-decode';
import api from '../../services/api';

const TokenDebugger = () => {
  const { token, isAuthenticated } = useContext(AuthContext);
  const [debugInfo, setDebugInfo] = useState(null);
  const [error, setError] = useState(null);
  const [loading, setLoading] = useState(false);

  const checkToken = () => {
    setLoading(true);
    setError(null);
    
    try {
      if (!token) {
        setError('No token found in AuthContext');
        setLoading(false);
        return;
      }
      
      // Decode token
      const decoded = jwtDecode(token);
      const now = Date.now() / 1000;
      const isExpired = decoded.exp < now;
      
      setDebugInfo({
        tokenFound: true,
        tokenIsValid: !isExpired,
        tokenExpiration: new Date(decoded.exp * 1000).toLocaleString(),
        currentTime: new Date(now * 1000).toLocaleString(),
        timeRemaining: isExpired ? 'Expired' : `${Math.floor((decoded.exp - now) / 60)} minutes, ${Math.floor((decoded.exp - now) % 60)} seconds`,
        tokenData: decoded
      });
    } catch (err) {
      setError(`Error decoding token: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  const testApiCall = async () => {
    setLoading(true);
    setError(null);
    
    try {
      // Make a test API call to check authorization
      const response = await api.get('/api/aws/resources?page=0&page_size=1');
      setDebugInfo(prev => ({
        ...prev,
        apiCallSuccess: true,
        apiResponse: {
          status: response.status,
          statusText: response.statusText,
          data: response.data
        }
      }));
    } catch (err) {
      setError(`API call failed: ${err.message}`);
      setDebugInfo(prev => ({
        ...prev,
        apiCallSuccess: false,
        apiError: {
          message: err.message,
          status: err.response?.status,
          statusText: err.response?.statusText,
          data: err.response?.data
        }
      }));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <i className="fa fa-bug"></i> Token Debugger
      </CardHeader>
      <CardBody>
        {error && <Alert color="danger">{error}</Alert>}
        
        <div className="mb-3">
          <Button 
            color="primary" 
            onClick={checkToken} 
            disabled={loading}
            className="me-2"
          >
            Check Token
          </Button>
          <Button 
            color="info" 
            onClick={testApiCall} 
            disabled={loading || !isAuthenticated}
          >
            Test API Call
          </Button>
        </div>
        
        {loading && <div className="text-center my-3">Loading...</div>}
        
        {debugInfo && (
          <div className="token-debug-info">
            <h5>Authentication Status</h5>
            <ul>
              <li>Token Found: {debugInfo.tokenFound ? 'Yes' : 'No'}</li>
              <li>Token Valid: {debugInfo.tokenIsValid ? 'Yes' : 'No (Expired)'}</li>
              <li>Token Expiration: {debugInfo.tokenExpiration}</li>
              <li>Current Time: {debugInfo.currentTime}</li>
              <li>Time Remaining: {debugInfo.timeRemaining}</li>
            </ul>
            
            {debugInfo.apiCallSuccess !== undefined && (
              <>
                <h5>API Call Test</h5>
                <ul>
                  <li>Success: {debugInfo.apiCallSuccess ? 'Yes' : 'No'}</li>
                  {debugInfo.apiCallSuccess ? (
                    <>
                      <li>Status: {debugInfo.apiResponse.status} {debugInfo.apiResponse.statusText}</li>
                      <li>Data: <pre>{JSON.stringify(debugInfo.apiResponse.data, null, 2)}</pre></li>
                    </>
                  ) : (
                    <>
                      <li>Error Status: {debugInfo.apiError.status} {debugInfo.apiError.statusText}</li>
                      <li>Error Data: <pre>{JSON.stringify(debugInfo.apiError.data, null, 2)}</pre></li>
                    </>
                  )}
                </ul>
              </>
            )}
            
            <h5>Token Data</h5>
            <pre className="token-data p-3 bg-light">
              {JSON.stringify(debugInfo.tokenData, null, 2)}
            </pre>
          </div>
        )}
      </CardBody>
    </Card>
  );
};

export default TokenDebugger;
