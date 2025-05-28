import React from 'react';

const Tab = ({ label, isActive, onClick }) => (
    <button
        style={{
            padding: '10px 15px', // Adjusted padding for a common use case
            marginRight: '5px',
            border: '1px solid #ccc',
            borderBottom: isActive ? 'none' : '1px solid #ccc',
            backgroundColor: isActive ? 'white' : '#f0f0f0',
            cursor: 'pointer'
        }}
        onClick={onClick}
    >
        {label}
    </button>
);

export default Tab;
