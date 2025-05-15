import React from "react";
import { Spinner as ReactstrapSpinner } from "reactstrap";

const Spinner = ({ text = "Loading..." }) => {
  return (
    <div className="text-center my-3">
      <ReactstrapSpinner color="primary" />
      <p className="mt-2">{text}</p>
    </div>
  );
};

export default Spinner;
