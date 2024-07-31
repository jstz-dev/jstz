import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { Jstz, User } from "@jstz-dev/sdk";

const DEFAULT_ENDPOINT = "localhost:8933";

const SignUp: React.FC<{ addUser: (name: string, user: User) => void }> = ({
  addUser,
}) => {
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [publicKey, setPublicKey] = useState("");
  const [secretKey, setSecretKey] = useState("");

  const signup = () => {
    const user: User = { address, publicKey, secretKey };
    addUser(name, user);
    alert("User signed up");
  };

  return (
    <div>
      <div>
        <label>Name:</label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <div>
        <label>Address:</label>
        <input
          type="text"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
        />
      </div>
      <div>
        <label>Public Key:</label>
        <input
          type="text"
          value={publicKey}
          onChange={(e) => setPublicKey(e.target.value)}
        />
      </div>
      <div>
        <label>Secret Key:</label>
        <input
          type="text"
          value={secretKey}
          onChange={(e) => setSecretKey(e.target.value)}
        />
      </div>
      <button onClick={signup}>Sign Up</button>
    </div>
  );
};

const LogIn: React.FC<{
  users: Map<string, User>;
  onLogin: (user: User) => void;
}> = ({ users, onLogin }) => {
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [publicKey, setPublicKey] = useState("");
  const [secretKey, setSecretKey] = useState("");

  const login = () => {
    const user = users.get(name);
    if (!user) {
      alert("User not found");
      return;
    }

    setAddress(user.address);
    setPublicKey(user.publicKey);
    setSecretKey(user.secretKey);
    onLogin(user);
  };

  return (
    <div>
      <div>
        <label>Name:</label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <button onClick={login}>Log In</button>
      <div>
        <div>Address: {address}</div>
        <div>Public Key: {publicKey}</div>
        <div>Secret Key: {secretKey}</div>
      </div>
    </div>
  );
};

const DeploySmartContract: React.FC<{ endpoint: string; user: User }> = ({
  endpoint,
  user,
}) => {
  const [code, setCode] = useState("");
  const [initialBalance, setInitialBalance] = useState(0);
  const [deployedAddress, setDeployedAddress] = useState("");

  const deployContract = async () => {
    const deployed = await new Jstz(endpoint).deploy(user, code, 0);
    setDeployedAddress(deployed);
  };

  return (
    <div>
      <div>
        <label>Initial Balance:</label>
        <input
          type="number"
          value={initialBalance}
          onChange={(e) => setInitialBalance(parseInt(e.target.value))}
        />
      </div>
      <div>
        <label>Code:</label>
      </div>
      <div>
        <textarea
          value={code}
          onChange={(e) => setCode(e.target.value)}
          style={{ height: "200px", width: "400px" }}
        ></textarea>
      </div>
      <div>
        <button onClick={deployContract}>Deploy Contract</button>
        <div>Deployed Address: {deployedAddress}</div>
      </div>
    </div>
  );
};

const RunSmartFunction: React.FC<{ endpoint: string; user: User }> = ({
  endpoint,
  user,
}) => {
  const [uri, setUri] = useState("");

  const [functionResult, setFunctionResult] = useState(0);

  const runFunction = async () => {
    const result = await new Jstz(endpoint).run(user, { uri });
    setFunctionResult(result.statusCode);
  };

  return (
    <div>
      <div>
        <label>URI:</label>
        <input
          type="text"
          value={uri}
          onChange={(e) => setUri(e.target.value)}
        />
      </div>

      <button onClick={runFunction}>Run Function</button>
      <div>Function Result: {functionResult}</div>
    </div>
  );
};

const Functions: React.FC<{ endpoint: string; user: User }> = (props) => {
  const [selectedOption, setSelectedOption] = useState("");

  return (
    <div>
      <div>
        <label>Operation: </label>
        <select
          value={selectedOption}
          onChange={(e) => setSelectedOption(e.target.value)}
        >
          <option value="">Select an option</option>
          <option value="getNonce">Get Nonce</option>
          <option value="deployContract">Deploy Contract</option>
          <option value="runFunction">Run Function</option>
        </select>
      </div>

      {selectedOption === "deployContract" && (
        <DeploySmartContract {...props} />
      )}
      {selectedOption === "runFunction" && <RunSmartFunction {...props} />}
    </div>
  );
};

const Banner: React.FC = () => {
  return (
    <div>
      <h1>👨‍⚖️ jstz dashboard</h1>
    </div>
  );
};

const App: React.FC = () => {
  const [users, setUsers] = useState<Map<string, User>>(new Map());
  const [loggedInUser, setLoggedInUser] = useState<User | null>(null);
  const [endpoint, setEndpoint] = useState(DEFAULT_ENDPOINT);

  const [selectedOption, setSelectedOption] = useState("");

  const addUser = (name: string, user: User) => {
    users.set(name, user);
    // Is this needed?
    setUsers(users);
  };

  const onLogin = (user: User) => {
    setLoggedInUser(user);
  };

  return (
    <div>
      <Banner />
      <div>
        <label>Endpoint: </label>
        <input
          type="text"
          value={endpoint}
          onChange={(e) => setEndpoint(e.target.value)}
        />
      </div>
      <div>
        {/* Super crude navbar for selecting components */}
        <select
          value={selectedOption}
          onChange={(e) => setSelectedOption(e.target.value)}
        >
          <option value="">Select an option</option>
          <option value="signup">Sign Up</option>
          <option value="login">Login</option>
          <option value="functions">Functions</option>
        </select>
        {selectedOption === "signup" && <SignUp addUser={addUser} />}
        {selectedOption === "login" && (
          <LogIn users={users} onLogin={onLogin} />
        )}
        {selectedOption === "functions" && loggedInUser && (
          <Functions endpoint={endpoint} user={loggedInUser} />
        )}
      </div>
    </div>
  );
};

const root = createRoot(document.getElementById("root") as HTMLElement);

root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
