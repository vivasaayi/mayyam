import React from 'react';
import { BrowserRouter as Router, Route, Switch } from 'react-router-dom';
import Navigation from './components/Navigation';
import RDSClustersTable from './components/RDSClustersTable';

const App = () => {
    return (
        <Router>
            <Navigation />
            <Switch>
                <Route path="/rds-clusters" component={RDSClustersTable} />
            </Switch>
        </Router>
    );
};

export default App;
