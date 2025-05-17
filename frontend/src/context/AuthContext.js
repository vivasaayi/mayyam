import React, { createContext, useState, useEffect } from "react";
import axios from "axios";
import { jwtDecode } from "jwt-decode";

export const AuthContext = createContext();

export const AuthProvider = ({ children }) => {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [user, setUser] = useState(null);
  const [token, setToken] = useState(localStorage.getItem("token"));
  const [isLoading, setIsLoading] = useState(true);

  // Initialize auth state from local storage
  useEffect(() => {
    const initAuth = async () => {
      const storedToken = localStorage.getItem("token");
      
      if (storedToken) {
        try {
          // Verify token is valid and not expired
          const decodedToken = jwtDecode(storedToken);
          const currentTime = Date.now() / 1000;
          
          if (decodedToken.exp > currentTime) {
            // Set auth axios header
            axios.defaults.headers.common["Authorization"] = `Bearer ${storedToken}`;
            setUser(decodedToken);
            setIsAuthenticated(true);
            setToken(storedToken);
          } else {
            // Token expired, clean up
            console.warn("Token has expired, logging out");
            logout();
          }
        } catch (error) {
          console.error("Invalid token:", error);
          logout();
        }
      }
      
      setIsLoading(false);
    };
    
    initAuth();
  }, []);

  const login = async (credentials) => {
    try {
      const response = await axios.post("/api/auth/login", credentials);
      const { token } = response.data;
      
      localStorage.setItem("token", token);
      axios.defaults.headers.common["Authorization"] = `Bearer ${token}`;
      
      const decodedToken = jwtDecode(token);
      setUser(decodedToken);
      setToken(token);
      setIsAuthenticated(true);
      
      return true;
    } catch (error) {
      console.error("Login failed:", error);
      return false;
    }
  };

  const logout = () => {
    localStorage.removeItem("token");
    delete axios.defaults.headers.common["Authorization"];
    setUser(null);
    setToken(null);
    setIsAuthenticated(false);
  };

  const register = async (userData) => {
    try {
      await axios.post("/api/auth/register", userData);
      return true;
    } catch (error) {
      console.error("Registration failed:", error);
      return false;
    }
  };

  return (
    <AuthContext.Provider
      value={{
        isAuthenticated,
        user,
        token,
        isLoading,
        login,
        logout,
        register
      }}
    >
      {children}
    </AuthContext.Provider>
  );
};
