import React from "react";
import { CSpinner } from "@coreui/react";

const LoadingFallback = () => {
  return (
    <div className="d-flex justify-content-center align-items-center" style={{ height: "100vh" }}>
      <CSpinner color="primary" />
    </div>
  );
};

export default LoadingFallback;
