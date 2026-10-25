#pragma once

#include <functional>
#include <memory>
#include <fstream>
#include <mutex>

#include "ALVR-common/packet_types.h"
#include "Settings.h"

#include "openvr_driver.h"

class Statistics;

class ClientConnection {
public:

	ClientConnection();
	~ClientConnection();

	void FECSend(uint8_t *buf, int len, uint64_t targetTimestampNs, uint64_t videoFrameIndex);
	void SendVideo(uint8_t *buf, int len, uint64_t targetTimestampNs);
 	void ProcessTimeSync(TimeSync data);
	float GetPoseTimeOffset();
	void OnFecFailure();
	std::shared_ptr<Statistics> GetStatistics();

	std::shared_ptr<Statistics> m_Statistics;

	uint32_t videoPacketCounter = 0;

	uint64_t m_RTT = 0;
	int64_t m_TimeDiff = 0;

	TimeSync m_reportedStatistics;
	uint64_t m_lastFecFailure = 0;
	static const uint64_t CONTINUOUS_FEC_FAILURE = 60 * 1000 * 1000;
	static const int INITIAL_FEC_PERCENTAGE = 10;
	static const int MAX_FEC_PERCENTAGE = 20;
	int m_fecPercentage = INITIAL_FEC_PERCENTAGE;

	// The Reed-Solomon codec only depends on the number of data and parity shards, which almost never
	// changes between frames: keep the last one instead of rebuilding its matrices for every frame.
	reed_solomon *m_rs = nullptr;
	int m_rsDataShards = 0;
	int m_rsParityShards = 0;

	uint64_t mVideoFrameIndex = 1;

	uint64_t m_LastStatisticsUpdate;
};
