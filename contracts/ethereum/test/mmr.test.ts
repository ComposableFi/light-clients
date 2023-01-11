const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("MMR contract", function () {
  let mmr;
  let owner;
  beforeEach(async () => {
    [owner] = await ethers.getSigners();
    const MMRLibContract = await ethers.getContractFactory("MMR");
    const MMRlib = await MMRLibContract.deploy();
    const MMRWrapper = await ethers.getContractFactory("MMRWrapper", {
      libraries: {
        MMR: MMRlib.address,
      },
    });
    mmr = await MMRWrapper.deploy();
  });

  it("Should construct correct mmr tree", async function () {
    /**
     * Appending 10 items will construct a Merkle Mountain Range like below
     *              15
     *       7             14
     *    3      6     10       13       18
     *  1  2   4  5   8  9    11  12   16  17
     */
    const testValues = [
      ethers.utils.formatBytes32String("0x0001"), // position 1
      ethers.utils.formatBytes32String("0x0002"), // position 2
      ethers.utils.formatBytes32String("0x0003"), // position 4
      ethers.utils.formatBytes32String("0x0004"), // position 5
      ethers.utils.formatBytes32String("0x0005"), // position 8
      ethers.utils.formatBytes32String("0x0006"), // position 9
      ethers.utils.formatBytes32String("0x0007"), // position 11
      ethers.utils.formatBytes32String("0x0008"), // position 12
      ethers.utils.formatBytes32String("0x0009"), // position 16
      ethers.utils.formatBytes32String("0x000a"), // position 17
    ];

    for (let i = 0; i < testValues.length; i++) {
      await mmr.append(testValues[i]);
    }

    let position1Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [1, ethers.utils.keccak256(ethers.utils.concat([testValues[0]]))]
      )
    );
    expect(position1Value).to.be.equal(await mmr.getNode(1));

    let position2Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [2, ethers.utils.keccak256(ethers.utils.concat([testValues[1]]))]
      )
    );
    expect(position2Value).to.be.equal(await mmr.getNode(2));

    let position4Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [4, ethers.utils.keccak256(ethers.utils.concat([testValues[2]]))]
      )
    );
    expect(position4Value).to.be.equal(await mmr.getNode(4));

    let position5Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [5, ethers.utils.keccak256(ethers.utils.concat([testValues[3]]))]
      )
    );
    expect(position5Value).to.be.equal(await mmr.getNode(5));

    let position8Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [8, ethers.utils.keccak256(ethers.utils.concat([testValues[4]]))]
      )
    );
    expect(position8Value).to.be.equal(await mmr.getNode(8));

    let position9Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [9, ethers.utils.keccak256(ethers.utils.concat([testValues[5]]))]
      )
    );
    expect(position9Value).to.be.equal(await mmr.getNode(9));

    let position11Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [11, ethers.utils.keccak256(ethers.utils.concat([testValues[6]]))]
      )
    );
    expect(position11Value).to.be.equal(await mmr.getNode(11));

    let position12Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [12, ethers.utils.keccak256(ethers.utils.concat([testValues[7]]))]
      )
    );
    expect(position12Value).to.be.equal(await mmr.getNode(12));

    let position16Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [16, ethers.utils.keccak256(ethers.utils.concat([testValues[8]]))]
      )
    );
    expect(position16Value).to.be.equal(await mmr.getNode(16));

    let position17Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [17, ethers.utils.keccak256(ethers.utils.concat([testValues[9]]))]
      )
    );
    expect(position17Value).to.be.equal(await mmr.getNode(17));

    let position3Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [3, position1Value, position2Value]
      )
    );
    expect(position3Value).to.be.equal(await mmr.getNode(3));
    let position6Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [6, position4Value, position5Value]
      )
    );
    expect(position6Value).to.be.equal(await mmr.getNode(6));
    let position10Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [10, position8Value, position9Value]
      )
    );
    expect(position10Value).to.be.equal(await mmr.getNode(10));
    let position13Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [13, position11Value, position12Value]
      )
    );
    expect(position13Value).to.be.equal(await mmr.getNode(13));
    let position18Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [18, position16Value, position17Value]
      )
    );
    expect(position18Value).to.be.equal(await mmr.getNode(18));
    let position7Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [7, position3Value, position6Value]
      )
    );
    expect(position7Value).to.be.equal(await mmr.getNode(7));
    let position14Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [14, position10Value, position13Value]
      )
    );
    expect(position14Value).to.be.equal(await mmr.getNode(14));
    let position15Value = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32", "bytes32"],
        [15, position7Value, position14Value]
      )
    );
    expect(position15Value).to.be.equal(await mmr.getNode(15));

    const size = 18;
    //keccak256(abi.encodePacked(size, keccak256(abi.encodePacked(size, peaks)))
    let root = ethers.utils.keccak256(
      ethers.utils.solidityPack(
        ["uint256", "bytes32"],
        [
          size,
          ethers.utils.keccak256(
            ethers.utils.solidityPack(
              ["uint256", "bytes"],
              [size, ethers.utils.concat([position15Value, position18Value])]
            )
          ),
        ]
      )
    );

    const data = await mmr.getMerkleProof(1);
    expect(data.root).to.be.equal(root);
  });

  it("Should verify correct proof", async function () {
    /**
     * Appending 10 items will construct a Merkle Mountain Range like below
     *              15
     *       7             14
     *    3      6     10       13       18
     *  1  2   4  5   8  9    11  12   16  17
     */
    const testValues = [
      ethers.utils.formatBytes32String("0x0001"),
      ethers.utils.formatBytes32String("0x0002"),
      ethers.utils.formatBytes32String("0x0003"),
      ethers.utils.formatBytes32String("0x0004"),
      ethers.utils.formatBytes32String("0x0005"),
      ethers.utils.formatBytes32String("0x0006"),
      ethers.utils.formatBytes32String("0x0007"),
      ethers.utils.formatBytes32String("0x0008"),
      ethers.utils.formatBytes32String("0x0009"),
      ethers.utils.formatBytes32String("0x000a"),
    ];
    for (let i = 0; i < testValues.length; i++) {
      await mmr.append(testValues[i]);
    }

    // create proof
    const index = 17;
    const data = await mmr.getMerkleProof(index);
    const root = data.root;
    const width = data.width;
    const peakBagging = data.peakBagging;
    const siblings = data.siblings;
    await mmr.verify(
      root,
      width,
      index,
      ethers.utils.formatBytes32String("0x000a"),
      peakBagging,
      siblings
    );
  });

  it("Should raise error on incorrect proof", async function () {
    /**
     * Appending 10 items will construct a Merkle Mountain Range like below
     *              15
     *       7             14
     *    3      6     10       13       18
     *  1  2   4  5   8  9    11  12   16  17
     */
    const testValues = [
      ethers.utils.formatBytes32String("0x0001"),
      ethers.utils.formatBytes32String("0x0002"),
      ethers.utils.formatBytes32String("0x0003"),
      ethers.utils.formatBytes32String("0x0004"),
      ethers.utils.formatBytes32String("0x0005"),
      ethers.utils.formatBytes32String("0x0006"),
      ethers.utils.formatBytes32String("0x0007"),
      ethers.utils.formatBytes32String("0x0008"),
      ethers.utils.formatBytes32String("0x0009"),
      ethers.utils.formatBytes32String("0x000a"),
    ];
    for (let i = 0; i < testValues.length; i++) {
      await mmr.append(testValues[i]);
    }

    // create proof
    const index = 17;
    const data = await mmr.getMerkleProof(index);
    const root = data.root;
    const width = data.width;
    const peakBagging = data.peakBagging;
    const siblings = data.siblings;
    await expect(
      mmr.verify(
        root,
        width,
        index,
        ethers.utils.formatBytes32String("0x0009"),
        peakBagging,
        siblings
      )
    ).to.be.revertedWith("Hashed peak is invalid");
  });
});
