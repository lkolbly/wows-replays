const socket = new WebSocket('ws://localhost:3000/subscribe');

socket.addEventListener('open', function (event) {
    socket.send('lkolbly');
});

let last_scores = [{ score: 0, time: 0 }, { score: 0, time: 0 }];
let point_status = [];
let hold_reward;

socket.addEventListener('message', function (event) {
    //console.log('Message from server ', event.data);

    let packet = JSON.parse(event.data);
    //console.log(packet);
    if (packet.payload.PropertyUpdate != undefined) {
        //console.log(packet);
        //console.log('Property update ', packet.payload.PropertyUpdate);

        if (packet.payload.PropertyUpdate.property == "state" &&
            packet.payload.PropertyUpdate.update_cmd.levels[0].DictKey == "missions" &&
            packet.payload.PropertyUpdate.update_cmd.levels[1].DictKey == "teamsScore") {
            let teamId = packet.payload.PropertyUpdate.update_cmd.levels[2].ArrayIndex;
            let score = packet.payload.PropertyUpdate.update_cmd.action.SetKey.value;
            //console.log(teamId, score);
            let delta_score = score - last_scores[teamId].score;
            let delta_t = packet.clock - last_scores[teamId].time;

            // Find all the points owned by this team
            let increase_rate = 0.0;
            for (let i = 0; i < point_status.length; i++) {
                if (point_status[i].teamId == teamId && point_status[i].hasInvaders == 0) {
                    increase_rate += hold_reward.reward / hold_reward.period;
                }
            }

            let time_to_win = (1000 - score) / increase_rate;

            let divname = teamId == 1 ? "team2_time" : "team1_time";
            document.getElementById(divname).innerHTML = "" + time_to_win;

            last_scores[teamId].score = score;
            last_scores[teamId].time = packet.clock;
        } else if (packet.payload.PropertyUpdate.property == "state"
            && packet.payload.PropertyUpdate.update_cmd.levels[0].DictKey == "controlPoints") {
            let pointNo = packet.payload.PropertyUpdate.update_cmd.levels[1].ArrayIndex;
            let property = packet.payload.PropertyUpdate.update_cmd.action.SetKey.key;
            let value = packet.payload.PropertyUpdate.update_cmd.action.SetKey.value;
            console.log(pointNo, property, value);
            point_status[pointNo][property] = value;
        }
    } else if (packet.payload.EntityCreate != undefined && packet.payload.EntityCreate.entity_type == "BattleLogic") {
        hold_reward = packet.payload.EntityCreate.props.state.missions.hold[0];
        console.log(hold_reward);

        let cps = packet.payload.EntityCreate.props.state.controlPoints;
        for (let i = 0; i < cps.length; i++) {
            point_status.push(cps[i]);
        }
    }
});
